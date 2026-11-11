//! Serial link to the counting hardware, run on a background thread.
//!
//! The port is opened synchronously in [`SerialConnection::open`] so configuration errors
//! (missing port, permissions, port held by another program such as the old VM) reach the
//! UI immediately. The worker thread then either:
//!
//! - drives the PHYWE Cobra as a command/response device: `X 20` handshake, then for each
//!   gate `Z` (hardware-timed gate) and `z 2` polling until the count arrives; or
//! - reads a pulse stream (ASCII lines or raw bytes) from simpler devices.
//!
//! Every event carries the time it was observed (milliseconds on the app clock), so gates
//! and pulses are attributed to the right time even if the UI is not repainting.

use alloc::sync::Arc;
use core::sync::atomic::{AtomicBool, Ordering};
use core::time::Duration;
#[cfg(unix)]
use std::io::{Read, Write};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::thread::{JoinHandle, sleep, spawn};
use std::time::Instant;

use curie_core::{
    COBRA_COUNTER_CHANNEL, CobraCommand, CobraEdge, CobraLineReader, CobraReply, Device,
    DeviceCommand, DeviceEvent,
};

/// Supported serial baud rates (the Cobra uses 9600 or 19200).
pub const SUPPORTED_BAUD_RATES: [u32; 5] = [9_600, 19_200, 38_400, 57_600, 115_200];

/// How long to wait for the Cobra to answer a command.
const REPLY_TIMEOUT: Duration = Duration::from_millis(1_500);
/// Interval between `z 2` polls once a gate should have finished.
const POLL_INTERVAL: Duration = Duration::from_millis(40);
/// Consecutive unanswered commands before the link is declared lost.
const MAX_MISSED_REPLIES: u32 = 3;

/// Device protocol on the serial link.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SerialProtocol {
    /// PHYWE Cobra: ASCII commands, hardware-timed gates (recovered from RADIO1/RADIO2).
    PhyweCobra,
    /// One decimal count per line (Arduino, GMC and similar counters).
    AsciiText,
    /// Every received byte is a pulse count (0-255).
    RawBytePulses,
}

/// Parameters of a serial connection.
#[derive(Debug, Clone)]
pub struct SerialConfig {
    pub port: String,
    pub baud: u32,
    /// Initialisation string for streaming devices (escapes `\r`, `\n`, `\t`, `\0`).
    pub init_cmd: String,
}

/// An open serial link and its worker thread.
pub struct SerialConnection {
    pub events: Receiver<DeviceEvent>,
    pub commands: Sender<DeviceCommand>,
    /// Set on drop to ask the worker to finish.
    pub stop_signal: Arc<AtomicBool>,
    /// Worker thread, joined on drop.
    pub worker: Option<JoinHandle<()>>,
}

impl SerialConnection {
    /// Opens and configures the port, then starts the worker thread.
    ///
    /// Events are timestamped in milliseconds since `clock`.
    pub fn open(
        config: &SerialConfig,
        protocol: SerialProtocol,
        clock: Instant,
    ) -> Result<Self, String> {
        let port = Port::open(&config.port, config.baud)?;
        let (event_tx, event_rx) = channel();
        let (command_tx, command_rx) = channel();
        let stop_signal = Arc::new(AtomicBool::new(false));
        let stop = Arc::clone(&stop_signal);
        let worker_config = config.clone();
        let worker = spawn(move || {
            let outcome = match protocol {
                SerialProtocol::PhyweCobra => run_cobra(
                    port,
                    worker_config.baud,
                    clock,
                    &event_tx,
                    &command_rx,
                    &stop,
                ),
                SerialProtocol::AsciiText | SerialProtocol::RawBytePulses => {
                    run_stream(port, &worker_config, protocol, clock, &event_tx, &stop)
                }
            };
            if let Err(message) = outcome {
                drop(event_tx.send(DeviceEvent::Failed(message)));
            }
        });
        Ok(Self {
            events: event_rx,
            commands: command_tx,
            stop_signal,
            worker: Some(worker),
        })
    }
}

impl Device for SerialConnection {
    fn send(&mut self, command: DeviceCommand, _now_ms: u64) {
        _ = self.commands.send(command);
    }

    fn poll(&mut self, _now_ms: u64) -> Option<DeviceEvent> {
        self.events.try_recv().ok()
    }
}

impl Drop for SerialConnection {
    fn drop(&mut self) {
        self.stop_signal.store(true, Ordering::SeqCst);
        if let Some(worker) = self.worker.take() {
            drop(worker.join());
        }
    }
}

/// Parses escape sequences like `\r` and `\n` into bytes.
#[must_use]
pub fn parse_escaped_string(text: &str) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        let mut utf8 = [0_u8; 4];
        if c == '\\' {
            match chars.next() {
                Some('r') => bytes.push(b'\r'),
                Some('n') => bytes.push(b'\n'),
                Some('t') => bytes.push(b'\t'),
                Some('\\') | None => bytes.push(b'\\'),
                Some('0') => bytes.push(0),
                Some(other) => {
                    bytes.push(b'\\');
                    bytes.extend_from_slice(other.encode_utf8(&mut utf8).as_bytes());
                }
            }
        } else {
            bytes.extend_from_slice(c.encode_utf8(&mut utf8).as_bytes());
        }
    }
    bytes
}

/// Lists likely serial ports on this machine.
#[must_use]
pub fn candidate_ports() -> Vec<String> {
    #[cfg(unix)]
    {
        let mut ports: Vec<String> = std::fs::read_dir("/dev").map_or_else(
            |_| Vec::new(),
            |entries| {
                entries
                    .filter_map(Result::ok)
                    .filter_map(|entry| entry.file_name().into_string().ok())
                    .filter(|name| {
                        name.starts_with("ttyUSB")
                            || name.starts_with("ttyACM")
                            || matches!(name.as_str(), "ttyS0" | "ttyS1" | "ttyS2" | "ttyS3")
                    })
                    .map(|name| format!("/dev/{name}"))
                    .collect()
            },
        );
        ports.sort();
        ports
    }
    #[cfg(not(unix))]
    {
        (1..=8).map(|n| format!("COM{n}")).collect()
    }
}

/// Default port: a USB adapter if present, otherwise the second built-in port (the PHYWE
/// software and `RTG.CFG` use COM2).
#[must_use]
pub fn default_port() -> String {
    #[cfg(unix)]
    {
        candidate_ports()
            .into_iter()
            .find(|port| port.contains("ttyUSB") || port.contains("ttyACM"))
            .unwrap_or_else(|| "/dev/ttyS1".to_owned())
    }
    #[cfg(not(unix))]
    {
        "COM2".to_owned()
    }
}

// ============================================================================
// Protocol workers
// ============================================================================

/// How one hardware gate ended.
enum GateEnd {
    /// The count was delivered.
    Counted,
    /// The gate was abandoned; continue with this job instead (`None` = wait for one).
    Replaced(Option<(u32, bool)>),
    /// Stop requested, or the UI side of the link is gone.
    Stop,
}

/// Drives the Cobra: handshake, then gates on request.
fn run_cobra(
    mut port: Port,
    baud: u32,
    clock: Instant,
    events: &Sender<DeviceEvent>,
    commands: &Receiver<DeviceCommand>,
    stop: &AtomicBool,
) -> Result<(), String> {
    let mut handshakes = vec![CobraCommand::Handshake { code: 20 }];
    if baud == 19_200 {
        handshakes.push(CobraCommand::Handshake { code: 21 });
    }
    for command in handshakes {
        match exchange(&mut port, command)? {
            Some(CobraReply::Ack) => {}
            Some(CobraReply::Error(code)) => {
                return Err(format!(
                    "Cobra rejected the handshake (firmware error {code})"
                ));
            }
            Some(other) => {
                drop(events.send(DeviceEvent::Warning(format!(
                    "Unexpected handshake reply {other:?}"
                ))));
            }
            None => {
                return Err(
                    "No answer from the Cobra to X 20: check power, cable, port and baud rate"
                        .to_owned(),
                );
            }
        }
    }
    drop(events.send(DeviceEvent::Ready));

    let mut job: Option<(u32, bool)> = None;
    while !stop.load(Ordering::SeqCst) {
        let Some((gate_ms, repeat)) = job else {
            match commands.recv_timeout(Duration::from_millis(100)) {
                Ok(DeviceCommand::StartGates {
                    gate_ms: next_gate_ms,
                    repeat: next_repeat,
                }) => job = Some((next_gate_ms, next_repeat)),
                Ok(DeviceCommand::StopGates) | Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return Ok(()),
            }
            continue;
        };
        match cobra_gate(&mut port, gate_ms, clock, events, commands, stop)? {
            GateEnd::Counted if !repeat => job = None,
            GateEnd::Counted => {}
            GateEnd::Replaced(next) => job = next,
            GateEnd::Stop => return Ok(()),
        }
    }
    Ok(())
}

/// Runs one hardware gate: `Z` to arm it, a wait that stays responsive to new commands,
/// then `z 2` polls until the Cobra reports the count.
fn cobra_gate(
    port: &mut Port,
    gate_ms: u32,
    clock: Instant,
    events: &Sender<DeviceEvent>,
    commands: &Receiver<DeviceCommand>,
    stop: &AtomicBool,
) -> Result<GateEnd, String> {
    let to_ms = |instant: Instant| {
        u64::try_from(instant.saturating_duration_since(clock).as_millis()).unwrap_or(u64::MAX)
    };
    let start = CobraCommand::StartGate {
        edge: CobraEdge::Mode1,
        gate_us: u64::from(gate_ms).saturating_mul(1_000),
    };
    match exchange(port, start)? {
        Some(CobraReply::Ack) => {}
        Some(CobraReply::Error(code)) => {
            drop(events.send(DeviceEvent::Warning(format!(
                "Cobra rejected the gate of {gate_ms} ms (firmware error {code})"
            ))));
            return Ok(GateEnd::Replaced(None));
        }
        Some(other) => {
            drop(events.send(DeviceEvent::Warning(format!(
                "Unexpected reply to Z: {other:?}"
            ))));
        }
        None => return Err("The Cobra stopped answering (Z command)".to_owned()),
    }
    let started = Instant::now();
    let due = started
        .checked_add(Duration::from_millis(u64::from(gate_ms)))
        .unwrap_or(started);

    // Wait for the gate, staying responsive to stop/abort requests.
    while Instant::now() < due {
        if stop.load(Ordering::SeqCst) {
            return Ok(GateEnd::Stop);
        }
        let remaining = due.saturating_duration_since(Instant::now());
        match commands.recv_timeout(remaining.min(Duration::from_millis(100))) {
            Ok(DeviceCommand::StopGates) => return Ok(GateEnd::Replaced(None)),
            Ok(DeviceCommand::StartGates {
                gate_ms: next_gate_ms,
                repeat,
            }) => return Ok(GateEnd::Replaced(Some((next_gate_ms, repeat)))),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return Ok(GateEnd::Stop),
        }
    }

    // Poll until the count arrives.
    let mut missed = 0_u32;
    loop {
        if stop.load(Ordering::SeqCst) {
            return Ok(GateEnd::Stop);
        }
        match commands.try_recv() {
            Ok(DeviceCommand::StopGates) => return Ok(GateEnd::Replaced(None)),
            Ok(DeviceCommand::StartGates {
                gate_ms: next_gate_ms,
                repeat,
            }) => return Ok(GateEnd::Replaced(Some((next_gate_ms, repeat)))),
            Err(_) => {}
        }
        let poll = CobraCommand::PollGate {
            channel: COBRA_COUNTER_CHANNEL,
        };
        match exchange(port, poll)? {
            Some(CobraReply::Count(count)) => {
                drop(events.send(DeviceEvent::Gate {
                    started_ms: to_ms(started),
                    at_ms: to_ms(Instant::now()),
                    gate_ms,
                    count,
                }));
                return Ok(GateEnd::Counted);
            }
            Some(CobraReply::Error(code)) => {
                drop(events.send(DeviceEvent::Warning(format!(
                    "Cobra error {code} while reading the gate"
                ))));
                return Ok(GateEnd::Replaced(None));
            }
            Some(_) => sleep(POLL_INTERVAL),
            None => {
                missed = missed.saturating_add(1);
                if missed >= MAX_MISSED_REPLIES {
                    return Err("The Cobra stopped answering (z 2 polls)".to_owned());
                }
            }
        }
    }
}

/// Sends one command and waits for the first meaningful reply line.
///
/// Echoed commands and blank lines are skipped. `Ok(None)` means no reply in time.
fn exchange(port: &mut Port, command: CobraCommand) -> Result<Option<CobraReply>, String> {
    let line = command
        .encode()
        .ok_or_else(|| format!("Command out of range: {command:?}"))?;
    // A fresh line reader per exchange: leftovers of an earlier reply are discarded.
    let mut reader = CobraLineReader::new();
    port.write_all(line.as_bytes())?;

    let now = Instant::now();
    let deadline = now.checked_add(REPLY_TIMEOUT).unwrap_or(now);
    let mut buffer = [0_u8; 64];
    while Instant::now() < deadline {
        let read = port.read(&mut buffer)?;
        for &byte in buffer.iter().take(read) {
            if let Some(reply) = reader.feed(byte)
                && reply != CobraReply::Unknown
            {
                return Ok(Some(reply));
            }
        }
    }
    Ok(None)
}

/// Reads a pulse stream and forwards timestamped counts.
fn run_stream(
    mut port: Port,
    config: &SerialConfig,
    protocol: SerialProtocol,
    clock: Instant,
    events: &Sender<DeviceEvent>,
    stop: &AtomicBool,
) -> Result<(), String> {
    let init = parse_escaped_string(&config.init_cmd);
    if !init.is_empty() {
        port.write_all(&init)?;
    }
    drop(events.send(DeviceEvent::Ready));

    let mut line = String::new();
    let mut buffer = [0_u8; 256];
    while !stop.load(Ordering::SeqCst) {
        let read = port.read(&mut buffer)?;
        if read == 0 {
            continue;
        }
        let at_ms = u64::try_from(clock.elapsed().as_millis()).unwrap_or(u64::MAX);
        let mut count = 0_u32;
        for &byte in buffer.iter().take(read) {
            match protocol {
                SerialProtocol::RawBytePulses => count = count.saturating_add(u32::from(byte)),
                SerialProtocol::AsciiText | SerialProtocol::PhyweCobra => {
                    if byte == b'\n' || byte == b'\r' {
                        // First non-negative integer of the line ("CPS: 12" gives 12).
                        let value = line
                            .split(|c: char| !c.is_ascii_digit())
                            .find(|token| !token.is_empty())
                            .and_then(|token| token.parse::<u32>().ok())
                            .unwrap_or(0);
                        count = count.saturating_add(value);
                        line.clear();
                    } else if line.len() < 128 {
                        line.push(char::from(byte));
                    }
                }
            }
        }
        if count > 0 {
            drop(events.send(DeviceEvent::Pulses { at_ms, count }));
        }
    }
    Ok(())
}

// ============================================================================
// Platform port: POSIX
// ============================================================================

#[cfg(unix)]
struct Port {
    file: std::fs::File,
    name: String,
}

#[cfg(unix)]
impl Port {
    fn open(name: &str, baud: u32) -> Result<Self, String> {
        use std::os::unix::fs::OpenOptionsExt;
        // Probe with O_NONBLOCK first: it cannot hang on DCD, and its error kind (missing
        // port, permissions) does not depend on the system language.
        let nonblocking = if cfg!(target_os = "linux") {
            0o4000
        } else {
            0x0004
        };
        if let Err(error) = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(nonblocking)
            .open(name)
        {
            let kind = error.kind();
            return Err(if kind == std::io::ErrorKind::NotFound {
                format!("{name} does not exist. Check the cable/adapter or pick another port.")
            } else if kind == std::io::ErrorKind::PermissionDenied {
                format!(
                    "No permission to open {name}. Add your user to the 'dialout' (or 'uucp') group and log in again."
                )
            } else if kind == std::io::ErrorKind::ResourceBusy {
                format!("{name} is in use by another program (is the old VM still running?).")
            } else {
                format!("Cannot open {name}: {error}")
            });
        }

        // `clocal` stops open() from blocking on DCD on real RS-232 ports, and
        // `min 0 time 1` makes reads return after 100 ms of silence.
        let flag = if cfg!(target_os = "macos") {
            "-f"
        } else {
            "-F"
        };
        let output = std::process::Command::new("stty")
            .args([
                flag,
                name,
                &baud.to_string(),
                "raw",
                "-echo",
                "cs8",
                "-cstopb",
                "-parenb",
                "clocal",
                "-crtscts",
                "-ixon",
                "-ixoff",
                "min",
                "0",
                "time",
                "1",
            ])
            .output()
            .map_err(|error| format!("Could not run stty to configure {name}: {error}"))?;
        if !output.status.success() {
            return Err(format!(
                "Cannot configure {name} at {baud} baud: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(name)
            .map_err(|error| format!("Cannot open {name}: {error}"))?;
        Ok(Self {
            file,
            name: name.to_owned(),
        })
    }

    fn read(&mut self, buffer: &mut [u8]) -> Result<usize, String> {
        match self.file.read(buffer) {
            Ok(read) => Ok(read),
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock
                        | std::io::ErrorKind::TimedOut
                        | std::io::ErrorKind::Interrupted
                ) =>
            {
                Ok(0)
            }
            Err(error) => Err(format!("Read error on {}: {error}", self.name)),
        }
    }

    fn write_all(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.file
            .write_all(bytes)
            .and_then(|()| self.file.flush())
            .map_err(|error| format!("Write error on {}: {error}", self.name))
    }
}

// ============================================================================
// Platform port: Windows (Win32)
// ============================================================================

#[cfg(windows)]
#[expect(
    non_snake_case,
    unsafe_code,
    clippy::upper_case_acronyms,
    reason = "Win32 declarations keep the names of the Windows SDK"
)]
mod win32_api {
    pub const GENERIC_READ: u32 = 0x8000_0000;
    pub const GENERIC_WRITE: u32 = 0x4000_0000;
    pub const OPEN_EXISTING: u32 = 3;
    pub const FILE_ATTRIBUTE_NORMAL: u32 = 0x0000_0080;
    pub const INVALID_HANDLE_VALUE: isize = -1;
    /// `fBinary | fDtrControl = DTR_CONTROL_ENABLE | fRtsControl = RTS_CONTROL_ENABLE`.
    /// DTR/RTS are asserted as on Linux (the original code cleared them).
    pub const DCB_FLAGS: u32 = 0x0001 | 0x0010 | 0x1000;

    #[repr(C)]
    pub struct DCB {
        pub DCBlength: u32,
        pub BaudRate: u32,
        pub flags: u32,
        pub wReserved: u16,
        pub XonLim: u16,
        pub XoffLim: u16,
        pub ByteSize: u8,
        pub Parity: u8,
        pub StopBits: u8,
        pub XonChar: i8,
        pub XoffChar: i8,
        pub ErrorChar: i8,
        pub EofChar: i8,
        pub EvtChar: i8,
        pub wReserved1: u16,
    }

    #[repr(C)]
    pub struct COMMTIMEOUTS {
        pub ReadIntervalTimeout: u32,
        pub ReadTotalTimeoutMultiplier: u32,
        pub ReadTotalTimeoutConstant: u32,
        pub WriteTotalTimeoutMultiplier: u32,
        pub WriteTotalTimeoutConstant: u32,
    }

    unsafe extern "system" {
        pub fn CreateFileA(
            lpFileName: *const i8,
            dwDesiredAccess: u32,
            dwShareMode: u32,
            lpSecurityAttributes: *mut (),
            dwCreationDisposition: u32,
            dwFlagsAndAttributes: u32,
            hTemplateFile: *mut (),
        ) -> isize;
        pub fn GetCommState(hDevice: isize, lpDCB: *mut DCB) -> i32;
        pub fn SetCommState(hDevice: isize, lpDCB: *mut DCB) -> i32;
        pub fn SetCommTimeouts(hDevice: isize, lpCommTimeouts: *mut COMMTIMEOUTS) -> i32;
        pub fn ReadFile(
            hFile: isize,
            lpBuffer: *mut u8,
            nNumberOfBytesToRead: u32,
            lpNumberOfBytesRead: *mut u32,
            lpOverlapped: *mut (),
        ) -> i32;
        pub fn WriteFile(
            hFile: isize,
            lpBuffer: *const u8,
            nNumberOfBytesToWrite: u32,
            lpNumberOfBytesWritten: *mut u32,
            lpOverlapped: *mut (),
        ) -> i32;
        pub fn CloseHandle(hObject: isize) -> i32;
        pub fn GetLastError() -> u32;
    }
}

#[cfg(windows)]
struct Port {
    handle: isize,
    name: String,
}

#[cfg(windows)]
#[expect(unsafe_code, reason = "the serial port is driven through Win32 calls")]
impl Port {
    fn open(name: &str, baud: u32) -> Result<Self, String> {
        let path = if name.starts_with(r"\\.\") {
            name.to_owned()
        } else {
            format!(r"\\.\{name}")
        };
        let c_path = alloc::ffi::CString::new(path)
            .map_err(|error| format!("Invalid port name {name}: {error}"))?;

        // SAFETY: c_path is a valid NUL-terminated string; the other pointers are null as
        // allowed by CreateFileA.
        let handle = unsafe {
            win32_api::CreateFileA(
                c_path.as_ptr(),
                win32_api::GENERIC_READ | win32_api::GENERIC_WRITE,
                0,
                core::ptr::null_mut(),
                win32_api::OPEN_EXISTING,
                win32_api::FILE_ATTRIBUTE_NORMAL,
                core::ptr::null_mut(),
            )
        };
        if handle == win32_api::INVALID_HANDLE_VALUE {
            // SAFETY: GetLastError has no preconditions.
            let code = unsafe { win32_api::GetLastError() };
            return Err(match code {
                2 | 3 => {
                    format!("{name} does not exist. Check the cable/adapter or pick another port.")
                }
                5 => format!("{name} is in use by another program (is the old VM still running?)."),
                other => format!("Cannot open {name} (Windows error {other})"),
            });
        }
        let port = Self {
            handle,
            name: name.to_owned(),
        };

        // SAFETY: handle is a valid open comm handle; dcb is a properly sized DCB.
        let configured = unsafe {
            let mut dcb: win32_api::DCB = core::mem::zeroed();
            dcb.DCBlength = u32::try_from(size_of::<win32_api::DCB>()).unwrap_or(28);
            if win32_api::GetCommState(handle, &raw mut dcb) == 0 {
                false
            } else {
                dcb.BaudRate = baud;
                dcb.ByteSize = 8;
                dcb.Parity = 0; // NOPARITY
                dcb.StopBits = 0; // ONESTOPBIT
                dcb.flags = win32_api::DCB_FLAGS;
                win32_api::SetCommState(handle, &raw mut dcb) != 0
            }
        };
        if !configured {
            return Err(format!("Cannot configure {name} at {baud} baud"));
        }

        let mut timeouts = win32_api::COMMTIMEOUTS {
            ReadIntervalTimeout: 20,
            ReadTotalTimeoutMultiplier: 0,
            ReadTotalTimeoutConstant: 100,
            WriteTotalTimeoutMultiplier: 0,
            WriteTotalTimeoutConstant: 500,
        };
        // SAFETY: handle is valid and timeouts is a properly initialised struct.
        _ = unsafe { win32_api::SetCommTimeouts(handle, &raw mut timeouts) };
        Ok(port)
    }

    fn read(&mut self, buffer: &mut [u8]) -> Result<usize, String> {
        let mut read_bytes = 0_u32;
        let length = u32::try_from(buffer.len()).unwrap_or(u32::MAX);
        // SAFETY: buffer is valid for `length` bytes and read_bytes is a valid out-pointer.
        let ok = unsafe {
            win32_api::ReadFile(
                self.handle,
                buffer.as_mut_ptr(),
                length,
                &raw mut read_bytes,
                core::ptr::null_mut(),
            )
        };
        if ok == 0 {
            return Err(format!("Read error on {}", self.name));
        }
        Ok(usize::try_from(read_bytes).unwrap_or(0))
    }

    fn write_all(&mut self, bytes: &[u8]) -> Result<(), String> {
        let mut written = 0_u32;
        let length = u32::try_from(bytes.len()).unwrap_or(u32::MAX);
        // SAFETY: bytes is valid for `length` bytes and written is a valid out-pointer.
        let ok = unsafe {
            win32_api::WriteFile(
                self.handle,
                bytes.as_ptr(),
                length,
                &raw mut written,
                core::ptr::null_mut(),
            )
        };
        if ok == 0 || written != length {
            return Err(format!("Write error on {}", self.name));
        }
        Ok(())
    }
}

#[cfg(windows)]
#[expect(unsafe_code, reason = "the handle is closed through Win32")]
impl Drop for Port {
    fn drop(&mut self) {
        // SAFETY: handle was returned by CreateFileA and is closed exactly once.
        _ = unsafe { win32_api::CloseHandle(self.handle) };
    }
}
