//! PHYWE Cobra interface: serial protocol commands and reply parsing.
//!
//! The protocol was recovered by static analysis of the original PHYWE DOS software
//! (`RADIO2-E.EXE`, same Cobra communication unit as `RADIO1`):
//!
//! - Commands are ASCII, one letter plus space-separated decimal arguments, terminated by a
//!   single carriage return (no line feed).
//! - Replies are CR-terminated lines: `*` acknowledges a command, `?NN` reports firmware
//!   error `NN`, and counter values are eight hexadecimal digits (32 bits).
//! - `X 20` configures the handshake at start-up (`X 21` is sent as well at 19200 baud).
//! - `Z <edge> <hi> <lo>` starts a hardware-timed counting gate of `hi * 65536 + lo`
//!   microseconds (`lo = 0` stands for 65536), between 0.0655 s and 4294.97 s.
//! - `z 2` polls the gate on counter 2: `X` or `Y` while it is not finished, or the 32-bit
//!   count once it is. `w 2` reads counter 2 directly.
//!
//! Which of `X` / `Y` means "still counting", and whether the GM tube is always on counter
//! 2, could not be decided from the binary alone and must be confirmed on the hardware.

use alloc::string::String;
use core::fmt::Write;

/// Command terminator expected by the Cobra firmware.
pub const COBRA_TERMINATOR: u8 = b'\r';
/// Shortest gate the firmware accepts, in microseconds.
pub const COBRA_MIN_GATE_US: u64 = 65_537_u64;
/// Longest gate the firmware accepts, in microseconds.
pub const COBRA_MAX_GATE_US: u64 = 4_294_967_296_u64;
/// Counter channel the PHYWE software polls for the GM tube.
pub const COBRA_COUNTER_CHANNEL: u8 = 2_u8;
/// Longest reply line kept; longer lines are truncated.
const MAX_LINE_LEN: usize = 64_usize;

/// Counting edge selector of the `Z` command (firmware "Flankenmodus").
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CobraEdge {
    /// Mode `1`, the value the PHYWE software sends by default.
    #[default]
    Mode1,
    /// Mode `2`.
    Mode2,
}

/// A command for the Cobra interface.
#[expect(
    variant_size_differences,
    reason = "Short-lived command values; boxing the gate variant would need an allocation"
)]
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CobraCommand {
    /// `X <code>`: handshake / echo configuration (`20`, and `21` at 19200 baud).
    Handshake {
        /// Handshake code.
        code: u8,
    },
    /// `Z <edge> <hi> <lo>`: start a hardware-timed counting gate.
    StartGate {
        /// Counting edge mode.
        edge: CobraEdge,
        /// Gate length in microseconds.
        gate_us: u64,
    },
    /// `z <channel>`: poll the gate result of a counter.
    PollGate {
        /// Counter channel.
        channel: u8,
    },
    /// `w <channel>`: read a counter.
    ReadCounter {
        /// Counter channel.
        channel: u8,
    },
}

impl CobraCommand {
    /// Encodes the command as sent on the wire, including the trailing CR.
    ///
    /// Returns `None` for a gate outside the firmware limits.
    #[must_use]
    pub fn encode(&self) -> Option<String> {
        let mut line = String::new();
        let written = match *self {
            Self::Handshake { code } => write!(line, "X {code}"),
            Self::StartGate { edge, gate_us } => {
                if !(COBRA_MIN_GATE_US..=COBRA_MAX_GATE_US).contains(&gate_us) {
                    return None;
                }
                // gate = hi * 65536 + lo, where the firmware reads lo = 0 as 65536.
                let mut high = gate_us >> 16_u32;
                let low = gate_us & 0xFFFF_u64;
                if low == 0_u64 {
                    high = high.saturating_sub(1_u64);
                }
                let edge_digit = match edge {
                    CobraEdge::Mode1 => 1_u8,
                    CobraEdge::Mode2 => 2_u8,
                };
                write!(line, "Z {edge_digit} {high} {low}")
            }
            Self::PollGate { channel } => write!(line, "z {channel}"),
            Self::ReadCounter { channel } => write!(line, "w {channel}"),
        };
        written.ok()?;
        line.push(char::from(COBRA_TERMINATOR));
        Some(line)
    }
}

/// A parsed reply line from the Cobra interface.
#[expect(
    variant_size_differences,
    reason = "A 32-bit count next to 1-byte error codes; the enum stays 8 bytes"
)]
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CobraReply {
    /// `*`: command accepted.
    Ack,
    /// `?NN`: firmware error number `NN`.
    Error(u8),
    /// `X`: gate status letter.
    StatusX,
    /// `Y`: gate status letter.
    StatusY,
    /// One to eight hexadecimal digits: a counter value.
    Count(u32),
    /// Anything else (for example an echoed command).
    Unknown,
}

impl CobraReply {
    /// Parses one reply line (without its CR); surrounding spaces are ignored.
    #[must_use]
    pub fn parse(line: &[u8]) -> Self {
        let trimmed = line.trim_ascii();
        match trimmed {
            [b'*', ..] => Self::Ack,
            [b'?', rest @ ..] => {
                let mut code = 0_u8;
                for &digit in rest.iter().take_while(|byte| byte.is_ascii_digit()).take(3) {
                    code = code
                        .saturating_mul(10_u8)
                        .saturating_add(digit.saturating_sub(b'0'));
                }
                Self::Error(code)
            }
            [b'X'] => Self::StatusX,
            [b'Y'] => Self::StatusY,
            digits
                if !digits.is_empty()
                    && digits.len() <= 8_usize
                    && digits.iter().all(u8::is_ascii_hexdigit) =>
            {
                let mut value = 0_u32;
                for &digit in digits {
                    let nibble = char::from(digit).to_digit(16_u32).unwrap_or(0_u32);
                    value = (value << 4_u32) | nibble;
                }
                Self::Count(value)
            }
            _ => Self::Unknown,
        }
    }
}

/// Assembles the Cobra's CR-terminated reply lines from received bytes.
///
/// Feed every received byte to [`Self::feed`]; line feeds are ignored and a line longer
/// than 64 bytes is truncated.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CobraLineReader {
    line: [u8; MAX_LINE_LEN],
    line_len: usize,
}

impl CobraLineReader {
    /// An empty reader.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            line: [0_u8; MAX_LINE_LEN],
            line_len: 0_usize,
        }
    }

    /// Feeds one received byte; returns the parsed reply when a CR completes a line.
    pub fn feed(&mut self, byte: u8) -> Option<CobraReply> {
        match byte {
            b'\n' => None,
            COBRA_TERMINATOR => {
                let reply = CobraReply::parse(self.line.get(..self.line_len).unwrap_or(&[]));
                self.line_len = 0_usize;
                Some(reply)
            }
            other => {
                if let Some(slot) = self.line.get_mut(self.line_len) {
                    *slot = other;
                    self.line_len = self.line_len.saturating_add(1_usize);
                }
                None
            }
        }
    }
}

impl Default for CobraLineReader {
    fn default() -> Self {
        Self::new()
    }
}
