# Curie-DAQ

[![Rust](https://img.shields.io/badge/rust-1.95%2B-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Target](https://img.shields.io/badge/targets-Linux%20%7C%20Windows%207%2B-informational.svg)]()

**Curie-DAQ** is a general-purpose data-acquisition and analysis application for Geiger-Müller counting
and nuclear radiation physics experiments. It supports the legacy **PHYWE Cobra** interface as well as
streaming serial devices, providing a modern cross-platform alternative to legacy DOS software
such as PHYWE `RADIO1`.

> **Status:** the Cobra protocol was recovered from the original PHYWE software and is tested
> against a simulated device, but it has **not yet been confirmed on the real interface**.
> See [`TODO.md`](TODO.md).

---

## What it does

* **PHYWE Cobra over RS-232 / USB-serial** (9600 or 19200 baud, 8N1). The Cobra times every gate
  itself (`Z` command, µs resolution), so counts per gate do not depend on the PC or on the
  window being visible.
* **Other counters:** ASCII line streams (Arduino, GMC…) and raw byte streams, timed by the PC with
  fixed gate deadlines.
* **Simulator** for practising without a source: Poisson counts, optional half-life (Pa-234m preset)
  and an attenuation bench model.
* **Gate series (1:ACQUIRE):** counts per gate Δt, any number of gates (RADIO1 was limited to 1000),
  live mean ± standard error, variance and Fano factor, rate plot and an integer histogram with the
  Poisson expectation.
* **Manual sweeps (2:SWEEP):** one gate per position and state, distance (mm) **or** angle (°),
  any number of bench states (named by you, e.g. *No filter* / *Al cap*, and picked with
  PREV / NEXT), heterogeneous spacing, REPEAT and UNDO.
* **Analysis (3:ANALYSIS):**
  * **Decay:** Poisson maximum-likelihood fit of `A·e^(−λ(t−t0)) + B` on the raw counts, with the
    background fitted or fixed to a measured zero effect. Reports λ, T½, R(t0), B **with
    uncertainties** and χ²/ndf. Gates at the background level are kept, so a long run does not
    bias T½.
  * **Gaussian / Poisson:** estimates from the raw counts (μ, σ, FWHM, λ, with errors) and a
    Pearson χ² on integer bins merged until each expects ≥ 5 gates.
  * Every formula, uncertainty and convention is explained in [`STATISTICS.md`](STATISTICS.md).
* **Nothing is lost:** every gate is appended to a CSV in the autosave folder (default
  `~/curie_runs`) and flushed immediately. Runs kept with SAVE RUN are stored in
  `~/curie_runs/saved_runs` and listed again the next time the app starts. Saved runs and exports
  carry a metadata header (start
  time, source, gate, detector, dead time).

---

## Running

```bash
# Linux
cargo run --release -p curie-ui

# Windows 10/11 binary built from Linux (needs mingw-w64)
rustup target add x86_64-pc-windows-gnu
cargo build --release --target x86_64-pc-windows-gnu -p curie-ui
# -> target/x86_64-pc-windows-gnu/release/curie-ui.exe

# Windows 7/8 (old lab PCs): the win7 targets, built with nightly and build-std
rustup component add rust-src --toolchain nightly
cargo +nightly build --release -Z build-std=std,panic_abort --target i686-win7-windows-gnu -p curie-ui
cargo +nightly build --release -Z build-std=std,panic_abort --target x86_64-win7-windows-gnu -p curie-ui
# -> target/<target>/release/curie-ui.exe (i686 = 32-bit Windows, any CPU with SSE2)
```

Requirements: Rust 1.95 or newer (needed by egui 0.37; `curie-core` alone builds with 1.88), OpenGL 2.0 (Mesa software rendering
is fine). The standard `*-pc-windows-gnu` targets need Windows 10; the `*-win7-windows-gnu`
targets (Rust tier 3, hence nightly) run on Windows 7 and later, which needs the Universal C
Runtime (update KB2999226 or a Visual C++ 2015+ redistributable) on an un-updated PC.

**Linux serial permissions:** add your user to the `dialout` group (`uucp` on Arch) and log in again:

```bash
sudo usermod -a -G dialout $USER
```

If another program or VM holds the serial port, close it first. The app reports
missing ports, missing permissions and busy ports explicitly in **4:CONFIG**.

---

## Typical experimental workflows

1. **Hardware connection:** **4:CONFIG** → *DEVICE* *PHYWE COBRA* (or *ASCII LINES* / *RAW BYTES*
   for a streaming counter), select the port (e.g. `/dev/ttyUSB0`, `/dev/ttyS0`, `COM1`, etc.) and
   baud rate (e.g. 9600 or 19200), and click **CONNECT**. The status must read *PHYWE COBRA ready*.
   Pick the detector there too (presets, or *CUSTOM* with a measured dead time).
2. **Background & Poisson statistics:** 1:ACQUIRE, Δt ≈ 10 s, choose the number of gates
   (e.g. 100, 500, 1000), **START**. Use *DISTRIBUTION* to view the live histogram; inspect
   3:ANALYSIS → *Gaussian* / *Poisson* for μ, σ, λ with uncertainties and Pearson χ².
3. **Distance sweeps & attenuation (e.g. inverse square law, absorption):** 2:SWEEP, axis *DISTANCE*,
   states named in *STATE NAMES…* (e.g. *No filter* / *Al cap*), Δt ≈ 20 s. Enter the distance
   before each **START STEP**; the table displays S1 − S2 ± σ.
4. **Half-life & exponential decay (e.g. Pa-234m):** 1:ACQUIRE with Δt ≈ 5–10 s, start acquisition,
   run across multiple half-lives. In 3:ANALYSIS → *Exponential*, set *START AT* to the start time (t0)
   and either fit the background or fix it to a measured background level.
5. **Angular distributions & deflection:** 2:SWEEP, axis *ANGLE*, e.g. −60° … +60° in 10° steps, Δt ≥ 30 s.

PAUSE discards the gate in progress; the time axis keeps running (correct for decay curves).

---

## CSV format

```csv
# Curie-DAQ gate series
# started_utc: 2026-11-03T10:15:02Z
# source: PHYWE COBRA on /dev/ttyS1 @ 9600 baud (gates timed by the device)
# gate_s: 10
# detector: PHYWE 09005.00 | dead_time_us: 100.0 | tube_voltage_V (operator note): 500
# t_mid_s = gate centre since start; rate = counts/gate_s; rate_sigma = sqrt(max(counts,1))/gate_s
index,t_mid_s,gate_s,counts,rate_cps,rate_sigma_cps,rate_deadtime_corrected_cps
0,5.030,10,9,0.900000,0.300000,0.900081
```

Sweep files have one row per step:
`step,axis,position,unit,state_index,state_label,counts,gate_s,rate_cps,rate_sigma_cps,rate_deadtime_corrected_cps`
(only the coordinate being swept: mm for distance sweeps, degrees for angle sweeps).

---

## Workspace

```
crates/
├── curie-core/   # #![no_std] + alloc, zero dependencies
│   ├── acquisition   gate sessions with fixed deadlines and hardware-gate ingestion
│   ├── analysis/     decay ML fit, Gaussian/Poisson fits, Pearson chi-squared
│   ├── hardware/     Device contract, Cobra protocol, MCU counter, detector presets/custom
│   ├── math          correctly rounded f64 sqrt/exp/ln/normal CDF/Poisson pmf
│   ├── stats/        Welford sample moments, integer-aligned histograms
│   ├── storage       datasets (with gate duration) and registry
│   └── sweep         position-by-state sweeps (distance or angle)
└── curie-ui/     # eframe/egui desktop app: serial worker, pages, autosave/export
```

The `f64` special functions in `curie-core` are evaluated in double-double arithmetic and
rounded once; `tests/f64_math_tests.rs` checks them bit-for-bit against `mpmath` references.

```bash
cargo test --workspace
cargo clippy --workspace --all-targets
```

---

## License

Apache-2.0. Author: Pedro Martins.
