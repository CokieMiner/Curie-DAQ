# Curie-DAQ architecture

Curie-DAQ is a modern data-acquisition and analysis application for Geiger-Müller counting and
nuclear radiation physics experiments (replacing legacy DOS software like PHYWE `RADIO1`). It is a
Cargo workspace with two crates:

| crate | role |
|---|---|
| `curie-core` | `#![no_std]` + `alloc`, **zero dependencies**: Cobra protocol, gate sessions, sweeps, datasets, statistics, fits, double-double math |
| `curie-ui` | eframe/egui desktop app: serial worker thread, acquisition engine, pages, autosave, saved runs and CSV export |

Both crates build under the strict workspace lints (`clippy::pedantic`, `nursery` and a large
restriction set). Exceptions are scoped `#[expect(..., reason = "...")]` attributes, never blanket
allows. The statistics behind every displayed number are explained in [`STATISTICS.md`](STATISTICS.md).

---

## 1. The PHYWE Cobra protocol

Recovered by static analysis of the original PHYWE software (`RADIO2-E.EXE`, which shares the Cobra
communication unit with `RADIO1`; the `X 20` / `X 21` handshake also appears in `RADIO1-E.OVR`).
Implemented in `curie-core/src/hardware/phywe.rs`.

| command (CR-terminated) | meaning | reply |
|---|---|---|
| `X 20` (+ `X 21` at 19200 baud) | handshake / echo configuration | `*` |
| `Z <edge> <hi> <lo>` | start a gate of `hi·65536 + lo` µs (`lo = 0` means 65536), 0.0655 s … 4294.97 s | `*` |
| `z 2` | poll the gate on counter 2 | `X` / `Y` while not finished, then 8 hex digits (32-bit count) |
| `w 2` | read counter 2 | 8 hex digits |

Errors come back as `?NN`, an index into the firmware error table found in the binary
(`Befehl zu lange`, `unzulässige Torzeit-Dauer beim Befehl "Z"`, …). Replies are CR-terminated; line
feeds and echoed commands are ignored.

The original software timestamps gates with the BIOS tick (55 ms), which is why RADIO1 files show
`10.05, 20.11, 30.21 s` for 10 s gates: the gate itself is exact, readout adds a short dead gap.

Still to confirm on the hardware (see `TODO.md`): which of `X`/`Y` means "still counting", that the GM
tube is on counter 2, and whether `Z` must be re-sent for every gate (Curie-DAQ re-sends it).

---

## 2. Timing model

Counts per gate are the primary data of radiation counting experiments, so gate timing must not depend on the UI.

Every counting device implements `curie_core::Device`: the engine sends
`DeviceCommand::StartGates { gate_ms, repeat }` / `StopGates` and polls `DeviceEvent`s, all
timestamped in milliseconds on the app clock. A device does one of two things:

* **Times its own gates** (Cobra, simulator) and answers with
  `DeviceEvent::Gate { started_ms, at_ms, gate_ms, count }`. The app records the gate centre as
  `started + gate/2`. The Cobra worker sends `Z`, waits for the gate and polls `z 2`; the simulator
  draws one Poisson count per gate on fixed deadlines, so a stalled UI catches up with correctly
  sized gates. Minimising the window or a stalled frame only delays the display.
* **Streams pulses** (ASCII lines, raw bytes) as `DeviceEvent::Pulses { at_ms, count }`.
  `CurieSession` has fixed deadlines (`start + n·Δt`); for each event the app first closes the
  gates that ended before its timestamp, then adds the pulses. Gates are closed 150 ms after their
  deadline to let pulses in flight arrive.

The engine only asks `DeviceKind::times_gates()`; nothing else in `app.rs` or the pages depends
on the device.

**Pause** discards the gate in progress; the run clock keeps running, so `t` stays physical time
(needed for decay curves). Acquisition and sweeps own the gates exclusively (`GateOwner`).

Every gate is appended to a CSV in the autosave folder and flushed immediately (`export::RunLog`).
Autosave is always on; the top bar shows NOT AUTOSAVING only when that file could not be written.

---

## 3. Core modules

```
acquisition.rs  CurieSession (fixed-deadline tick, ingest_gate for hardware gates), DataRecord
analysis/       ExponentialDecayFit, GaussianFit, PoissonFit, GoodnessOfFit
hardware/       Device contract (DeviceCommand, DeviceEvent); Cobra protocol + CobraLineReader;
                McuPulseCounter; detector presets / custom tube and HardwareConfig
math.rs         F64Math (sqrt, exp, ln, normal_cdf, poisson_pmf)
stats/          SampleMoments (Welford), IntegerHistogram + BinWidthMode
storage.rs      Dataset, StoredPoint, DatasetRegistry
sweep.rs        SweepSession (distance or angle axis, finish_step for hardware gates), groups
```

**API style.** Plain data has public fields and no accessors: `Dataset`, `StoredPoint`,
`SweepGroup`, `SweepRecord`, `CurieSession`, the fit results. Types with invariants keep their
fields private and expose methods: `SweepSession` (a state change cancels the running gate),
`DatasetRegistry` (unique ids, valid selection) and `HardwareConfig` (the bias stays on the
detector's plateau). Functions shorter than about five lines are inlined at the call site.

**Hardware abstraction.** Counting electronics and detector tubes are separate. A counting device
implements `Device` (see §2): the desktop app has the serial worker (`SerialConnection`, for the
Cobra and streaming counters) and the simulator, and the core has `McuPulseCounter` for a
microcontroller counter fed from a pin interrupt or a timer capture. A tube is a
`DetectorProfile`: `DetectorProfile::PRESETS` lists the known tubes and `DetectorProfile::custom`
takes a dead time measured in the lab; `HardwareConfig` keeps the noted bias on its plateau.

To add hardware:

* **A streaming serial counter** (one count per line, or raw bytes): nothing to write; pick
  ASCII LINES or RAW BYTES in 4:CONFIG.
* **Another serial device that times its own gates:** a `SerialProtocol` variant, its worker
  function in `serial.rs` emitting `DeviceEvent::Gate`, and a `DeviceKind` entry (`ALL`, `label`,
  `times_gates`) in `device.rs`.
* **A non-serial device** (USB HID, network): implement `Device` for it and add a `DeviceKind`
  variant and its arm in `CurieApp::device`.
* **A new tube:** a `DetectorProfile` constant in `PRESETS`, or CUSTOM in 4:CONFIG.

### 3.1 Statistics and fits

Sample moments, integer-aligned histograms, the Poisson and Gaussian fits, Pearson χ², the
Poisson maximum-likelihood decay fit and the uncertainty conventions are described, with their
formulas, in [`STATISTICS.md`](STATISTICS.md).

### 3.2 Dependency-free double precision

`core` has no `f64::exp/ln/sqrt` without `std`, and the crate has no dependencies, so `F64Math`
implements them:

* `sqrt`: integer square root of the mantissa, correctly rounded.
* `exp`, `ln`, `normal_cdf` (via `erfc`), `poisson_pmf` (via `ln k!`): evaluated in double-double
  (Dekker two-sum/two-product, no FMA, never more than two `f64` words per value) and rounded once.
  Subnormal results are rounded once as well.

On 175 000 random inputs checked against `mpmath` (80 digits) every function returned the correctly
rounded result; `tests/f64_math_tests.rs` keeps a bit-exact sample of those vectors.

---

## 4. Desktop app (`curie-ui`)

| file | role |
|---|---|
| `device.rs` | `DeviceKind`: the devices offered in 4:CONFIG and whether each times its own gates |
| `serial.rs` | `SerialConnection` (a `Device`): synchronous port open with explicit errors (missing, permission, busy), POSIX `stty … clocal` / Win32 DCB with DTR+RTS, worker thread for Cobra request/response or streams |
| `app.rs` | acquisition engine (`pump`), gate ownership, autosave, sweeps, saved runs, cached analysis, decay-fit worker |
| `export.rs` | `RunLog` autosave, CSV export with metadata header, saved-run reader, UTC timestamps |
| `simulation.rs` | `SimulatedCounter` (a `Device` that times its gates): Poisson counts, optional half-life, attenuation bench model |
| `pages/` | 1:ACQUIRE, 2:SWEEP, 3:ANALYSIS, 4:CONFIG |
| `tests/` | unit tests of the app (engine, CSV, serial helpers, plot downsampling), kept out of the source files |

### 4.1 Saved runs

SAVE RUN (1:ACQUIRE) and SAVE DATASET (2:SWEEP) add the run to the list in 3:ANALYSIS / 4:CONFIG and
write a copy to `<autosave folder>/saved_runs/<UTC time>_<id>_<name>.csv`. The file is an ordinary
export (metadata header, column row, one row per gate or step) with a first line
`# saved_run: <name>`; at start-up every such file is read back (`export::load_saved_run`) in name
order, which is chronological. Deleting a run removes its copy; the autosave logs of the original
measurement are never touched.

### 4.2 Analysis

Distribution fits (histogram, Gaussian, Poisson) are cached by their inputs and recomputed only when
those change. The decay fit runs on a worker thread: at most one at a time, and when it finishes
with outdated inputs the latest inputs start a new one. The previous result stays on screen, marked
"updating...", so dragging a parameter never blocks the UI (the double-double functions cost about
1–7 µs per call, which matters for series of 10⁴ gates).

### 4.3 Sweeps

A sweep steps one axis only (distance in mm or angle in degrees) and writes only that coordinate.
The axis and the gate time are locked once the first point exists, so a saved sweep (one gate
length for all steps) reloads exactly. States are unnamed by default (`S1`, `S2`, …); the
operator names them in the STATE NAMES window, and PREV / NEXT pick the state measured next, so
dozens of states fit in one row.

### 4.4 Windows builds

All Windows binaries are cross-built from Linux with mingw-w64; `.cargo/config.toml` links libgcc
statically, so they depend only on system DLLs (the Universal C Runtime `api-ms-win-crt-*`,
OpenGL32, user32, …).

| target | runs on | build |
|---|---|---|
| `x86_64-pc-windows-gnu` | Windows 10/11 (imports `api-ms-win-core-synch-l1-2-0` and `bcryptprimitives`, absent on 7) | stable |
| `i686-pc-windows-gnu` | Windows 10/11, 32-bit | stable |
| `x86_64-win7-windows-gnu` | Windows 7 and later, 64-bit | nightly, `-Z build-std` (tier 3) |
| `i686-win7-windows-gnu` | Windows 7 and later, 32-bit (SSE2, i.e. Pentium 4 or newer) | nightly, `-Z build-std` (tier 3) |

On Windows 7 the Universal C Runtime must be installed (update KB2999226, or any Visual C++
2015+ redistributable); an un-updated PC fails to start the program with a missing
`api-ms-win-crt-*.dll`. The app also needs OpenGL 2.0: on old graphics drivers that only offer
the Windows OpenGL 1.1, Mesa's software `opengl32.dll` placed next to the `.exe` works. The
serial code calls Win32 through `extern "system"` with pointer-sized handles, which is correct
for 32 and 64 bits.

---

## 5. Verification

* `curie-core`: strict workspace lints (`clippy::pedantic`, `nursery`, restriction set) and
  integration tests for the protocol encoding/parsing, the `Device` contract (MCU counter),
  detector presets and bias limits, fixed-deadline gates, pause semantics,
  hardware-gate ingestion, sweeps (including the angle axis), histograms, moments, all fits
  (including an unbiasedness check of the decay fit on synthetic Pa-234m runs) and correct rounding
  of `F64Math`.
* `curie-ui` (`src/tests/`): engine tests without a window (stalled UI, pause, stream timestamps,
  Cobra gate centre, a sweep step timed by the simulated device), saved runs surviving a restart
  and being deleted with their file, the decay-fit worker, CSV formatting (including the empty-gate
  σ) and file-name sanitising. The serial worker was exercised end to end against a simulated
  Cobra on a pseudo-terminal.
* Both crates' tests also pass as 32-bit Windows binaries (`i686-pc-windows-gnu` under Wine),
  including the bit-exact `F64Math` vectors; clippy is clean for `x86_64-pc-windows-gnu` (the
  Win32 serial code) and `curie-core` builds for a bare-metal target (`thumbv7em-none-eabi`).
