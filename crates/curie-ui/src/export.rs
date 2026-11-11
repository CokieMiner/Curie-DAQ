//! CSV files: incremental autosave of running measurements and on-demand export.
//!
//! Every file starts with `#` comment lines describing the run (start time, source, gate,
//! detector), followed by one header row and one row per counting gate, so a file is
//! usable on its own in Python, Excel or Origin without the lab notebook.

use std::fs::{File, create_dir_all};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use curie_core::{
    Dataset, DatasetType, F64Math, StoredPoint, SweepAxis, SweepRecord, SweepSession,
};

/// Column header of temporal (gate series) files.
pub const GATE_COLUMNS: &str =
    "index,t_mid_s,gate_s,counts,rate_cps,rate_sigma_cps,rate_deadtime_corrected_cps";
/// Column header of sweep files.
pub const SWEEP_COLUMNS: &str = "step,axis,position,unit,state_index,state_label,counts,gate_s,rate_cps,rate_sigma_cps,rate_deadtime_corrected_cps";
/// Column header of saved sweep datasets (one row per step, position on the swept axis).
pub const SAVED_SWEEP_COLUMNS: &str =
    "index,position,state_index,counts,gate_s,rate_cps,rate_sigma_cps";
/// First comment line of a file in the saved-runs folder, followed by the run name.
pub const SAVED_RUN_PREFIX: &str = "saved_run: ";

/// An append-only CSV file flushed after every row, so a crash or an accidental close
/// never loses more than the gate in progress.
pub struct RunLog {
    /// File being written.
    pub path: PathBuf,
    /// Buffered writer, flushed after every row.
    pub writer: BufWriter<File>,
    /// Data rows written so far (the next row index).
    pub rows: u64,
}

impl RunLog {
    /// Creates `<dir>/<stem>.csv` (never overwriting: a numeric suffix is added) and writes
    /// the comment header and the column row.
    pub fn create(dir: &str, stem: &str, header: &[String], columns: &str) -> Result<Self, String> {
        let path = csv_path(Path::new(dir), stem, true)?;
        let file = File::create(&path)
            .map_err(|error| format!("Cannot create '{}': {error}", path.display()))?;
        let mut writer = BufWriter::new(file);
        for line in header {
            writeln!(writer, "# {line}").map_err(|error| error.to_string())?;
        }
        writeln!(writer, "{columns}").map_err(|error| error.to_string())?;
        writer.flush().map_err(|error| error.to_string())?;
        Ok(Self {
            path,
            writer,
            rows: 0,
        })
    }

    /// Appends one data row (a gate or a sweep step) and flushes it to disk.
    pub fn append(&mut self, row: &str) -> Result<(), String> {
        self.rows = self.rows.saturating_add(1);
        writeln!(self.writer, "{row}")
            .and_then(|()| self.writer.flush())
            .map_err(|error| format!("Autosave to '{}' failed: {error}", self.path.display()))
    }
}

/// `rate,rate_sigma,rate_deadtime_corrected` columns of `counts` in a gate of `gate_s`.
fn rate_columns(counts: u32, gate_s: f64, dead_time_s: f64) -> String {
    let rate = f64::from(counts) / gate_s;
    // Poisson: sigma(N) = sqrt(N), with sqrt(1) for N = 0 so an empty gate is not exact.
    let sigma = F64Math::sqrt(f64::from(counts.max(1))) / gate_s;
    let live_fraction = rate.mul_add(-dead_time_s, 1.0);
    let corrected = if live_fraction > 0.0 {
        rate / live_fraction
    } else {
        f64::NAN
    };
    format!("{rate:.6},{sigma:.6},{corrected:.6}")
}

/// One CSV row of a counting gate.
pub fn gate_row(index: u64, t_mid_s: f64, gate_s: f64, counts: u32, dead_time_s: f64) -> String {
    let rates = rate_columns(counts, gate_s, dead_time_s);
    format!("{index},{t_mid_s:.3},{gate_s},{counts},{rates}")
}

/// One CSV row of a sweep step.
pub fn sweep_row(record: &SweepRecord, axis: SweepAxis, label: &str, dead_time_s: f64) -> String {
    let gate_s = f64::from(record.duration_ms) / 1000.0;
    let rates = rate_columns(record.pulses, gate_s, dead_time_s);
    // Only the coordinate the sweep steps through is written.
    let (axis_name, position, unit) = if axis == SweepAxis::Angle {
        ("angle", i32::from(record.angle_deg), "deg")
    } else {
        ("distance", i32::from(record.distance_mm), "mm")
    };
    let clean_label = label.replace(',', ";");
    format!(
        "{},{axis_name},{position},{unit},{},{clean_label},{},{gate_s},{rates}",
        record.step_index, record.state_index, record.pulses
    )
}

/// Creates `dir` if needed and returns `<dir>/<name>.csv`.
///
/// Characters other than letters, digits, `-`, `_` and `.` become `_`, so a run name can
/// never point outside `dir`. With `keep_existing`, an existing file is never reused:
/// `_2`, `_3`, ... is appended.
pub fn csv_path(dir: &Path, file_name: &str, keep_existing: bool) -> Result<PathBuf, String> {
    create_dir_all(dir)
        .map_err(|error| format!("Cannot create folder '{}': {error}", dir.display()))?;
    let stem: String = file_name
        .strip_suffix(".csv")
        .unwrap_or(file_name)
        .chars()
        .map(|character| {
            if character.is_alphanumeric() || matches!(character, '-' | '_' | '.') {
                character
            } else {
                '_'
            }
        })
        .collect();
    let mut path = dir.join(format!("{stem}.csv"));
    let mut suffix = 2_u32;
    while keep_existing && path.exists() {
        path = dir.join(format!("{stem}_{suffix}.csv"));
        suffix = suffix.saturating_add(1);
    }
    Ok(path)
}

/// Writes a saved dataset with its header lines to `path`.
///
/// Gate series get one row per gate; saved sweeps get one row per step.
pub fn export_dataset_csv_to(
    dataset: &Dataset,
    header: &[String],
    dead_time_s: f64,
    path: &Path,
) -> Result<PathBuf, String> {
    let gate_s = f64::from(dataset.gate_ms) / 1000.0;
    let (columns, rows): (&str, Vec<String>) = if dataset.kind == DatasetType::SweepMatrix {
        let rows = dataset
            .points
            .iter()
            .enumerate()
            .map(|(index, point)| {
                let sigma = F64Math::sqrt(f64::from(point.raw_count.max(1))) / gate_s;
                format!(
                    "{index},{},{},{},{gate_s},{},{sigma:.6}",
                    point.x, point.aux, point.raw_count, point.y
                )
            })
            .collect();
        (SAVED_SWEEP_COLUMNS, rows)
    } else {
        let rows = dataset
            .points
            .iter()
            .zip(0_u64..)
            .map(|(point, index)| {
                gate_row(
                    index,
                    f64::from(point.x),
                    gate_s,
                    point.raw_count,
                    dead_time_s,
                )
            })
            .collect();
        (GATE_COLUMNS, rows)
    };
    write_csv(path, header, columns, &rows)
}

/// Reads a file written to the saved-runs folder back into a dataset and its header lines.
pub fn load_saved_run(path: &Path) -> Result<(Dataset, Vec<String>), String> {
    let shown = path.display();
    let text = std::fs::read_to_string(path).map_err(|error| format!("{shown}: {error}"))?;
    let mut name = None;
    let mut header = Vec::new();
    let mut layout = None;
    let mut gate_s = 0.0_f64;
    let mut points = Vec::new();
    for line in text.lines() {
        if let Some(comment) = line.strip_prefix("# ") {
            match comment.strip_prefix(SAVED_RUN_PREFIX) {
                Some(saved_name) => name = Some(saved_name.to_owned()),
                None => header.push(comment.to_owned()),
            }
            continue;
        }
        let Some(kind) = layout else {
            layout = match line {
                GATE_COLUMNS => Some(DatasetType::AcquireTemporal),
                SAVED_SWEEP_COLUMNS => Some(DatasetType::SweepMatrix),
                _ => return Err(format!("{shown}: unknown column layout")),
            };
            continue;
        };
        // Both layouts keep x in column 1 and the counts in column 3; the gate is in
        // column 2 for gate series and column 4 for sweeps, whose state index is column 2.
        let fields: Vec<&str> = line.split(',').collect();
        let number = |column: usize| {
            fields
                .get(column)
                .and_then(|field| field.trim().parse::<f64>().ok())
        };
        let gate_column = if kind == DatasetType::SweepMatrix {
            4
        } else {
            2
        };
        let counts_field = fields
            .get(3)
            .and_then(|field| field.trim().parse::<u32>().ok());
        let (Some(x), Some(counts), Some(row_gate_s)) =
            (number(1), counts_field, number(gate_column))
        else {
            return Err(format!("{shown}: unreadable row '{line}'"));
        };
        gate_s = row_gate_s;
        let rate = f64::from(counts) / row_gate_s;
        let aux = if kind == DatasetType::SweepMatrix {
            number(2).unwrap_or_default()
        } else {
            F64Math::sqrt(f64::from(counts.max(1))) / row_gate_s
        };
        #[expect(
            clippy::as_conversions,
            clippy::cast_possible_truncation,
            reason = "stored points are f32 by design; the exact count is kept in raw_count"
        )]
        points.push(StoredPoint::new(x as f32, rate as f32, aux as f32, counts));
    }
    let (Some(kind), Some(run_name)) = (layout, name) else {
        return Err(format!("{shown}: not a saved run"));
    };
    let saved_at = std::fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |elapsed| elapsed.as_secs());
    let mut dataset = Dataset::new(&run_name, kind, saved_at);
    #[expect(
        clippy::as_conversions,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "gate durations are written from u32 milliseconds"
    )]
    let gate_duration_ms = (gate_s * 1000.0).round().clamp(0.0, f64::from(u32::MAX)) as u32;
    dataset.gate_ms = gate_duration_ms;
    dataset.points = points;
    Ok((dataset, header))
}

/// Writes every recorded sweep step to `path`.
pub fn export_sweep_csv_to(
    sweep: &SweepSession,
    labels: &[String],
    header: &[String],
    dead_time_s: f64,
    path: &Path,
) -> Result<PathBuf, String> {
    let rows: Vec<String> = sweep
        .points()
        .iter()
        .map(|record| {
            let label = labels
                .get(usize::from(record.state_index))
                .map_or("", String::as_str);
            sweep_row(record, sweep.axis(), label, dead_time_s)
        })
        .collect();
    write_csv(path, header, SWEEP_COLUMNS, &rows)
}

/// Writes header lines, the column row and the data rows to a new file at `path`.
fn write_csv(
    path: &Path,
    header: &[String],
    columns: &str,
    rows: &[String],
) -> Result<PathBuf, String> {
    let file = File::create(path)
        .map_err(|error| format!("Cannot create '{}': {error}", path.display()))?;
    let mut writer = BufWriter::new(file);
    let mut write_all = || -> std::io::Result<()> {
        for line in header {
            writeln!(writer, "# {line}")?;
        }
        writeln!(writer, "{columns}")?;
        for row in rows {
            writeln!(writer, "{row}")?;
        }
        writer.flush()
    };
    write_all().map_err(|error| format!("Writing '{}' failed: {error}", path.display()))?;
    Ok(path.to_path_buf())
}

/// Formats Unix seconds as `(ISO-8601 UTC, YYYYMMDD_HHMMSS)` without a date crate.
#[expect(
    clippy::integer_division,
    clippy::arithmetic_side_effects,
    reason = "Hinnant's civil-from-days relies on truncating division; days < 2^47 cannot overflow i64"
)]
pub fn utc_timestamp(unix_secs: u64) -> (String, String) {
    let days = i64::try_from(unix_secs / 86_400).unwrap_or(0);
    let seconds_of_day = unix_secs % 86_400;
    let (hour, minute, second) = (
        seconds_of_day / 3_600,
        seconds_of_day / 60 % 60,
        seconds_of_day % 60,
    );
    // Civil-from-days (Howard Hinnant's algorithm).
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (
        format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z"),
        format!("{year:04}{month:02}{day:02}_{hour:02}{minute:02}{second:02}"),
    )
}
