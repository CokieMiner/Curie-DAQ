//! Integration tests for the hardware abstraction: detectors, Cobra protocol and drivers.

use curie_core::{
    COBRA_COUNTER_CHANNEL, COBRA_MAX_GATE_US, COBRA_MIN_GATE_US, CobraCommand, CobraEdge,
    CobraLineReader, CobraReply, DeadTimeBasis, DetectorProfile, Device, DeviceCommand,
    DeviceEvent, HardwareConfig, McuPulseCounter,
};

#[test]
fn test_detector_presets_and_custom_tube() {
    let [phywe, pancake] = DetectorProfile::PRESETS;
    assert_eq!(phywe.name, "PHYWE 09005.00");
    assert_eq!(phywe.dead_time_sec.to_bits(), 100e-6_f32.to_bits());
    assert_eq!(phywe.dead_time_basis, DeadTimeBasis::Typical);
    assert_eq!(pancake.dead_time_sec.to_bits(), 20e-6_f32.to_bits());
    assert_eq!(pancake.dead_time_basis, DeadTimeBasis::DatasheetMinimum);
    assert_eq!(DeadTimeBasis::DatasheetMinimum.label(), "DATASHEET MIN");

    let custom = DetectorProfile::custom(55e-6_f32);
    assert_eq!(custom.name, DetectorProfile::CUSTOM_NAME);
    assert_eq!(custom.dead_time_sec.to_bits(), 55e-6_f32.to_bits());
    assert_eq!(custom.dead_time_basis, DeadTimeBasis::Measured);
}

#[test]
fn test_bias_voltage_stays_on_the_plateau() {
    let mut config = HardwareConfig::new(DetectorProfile::PHYWE_09005_00);
    assert_eq!(
        config.bias_voltage_v(),
        500_u16,
        "starts at the nominal voltage"
    );
    config.set_bias_voltage(10_000_u16);
    assert_eq!(config.bias_voltage_v(), 650_u16);
    config.set_bias_voltage(0_u16);
    assert_eq!(config.bias_voltage_v(), 425_u16);

    config.set_profile(DetectorProfile::PANCAKE_LND7311);
    assert_eq!(config.profile(), DetectorProfile::PANCAKE_LND7311);
    assert_eq!(
        config.bias_voltage_v(),
        900_u16,
        "a new detector resets to nominal"
    );
    config.set_bias_voltage(400_u16);
    assert_eq!(config.bias_voltage_v(), 850_u16);
}

#[test]
fn test_cobra_command_encoding_matches_phywe_software() {
    let handshake = CobraCommand::Handshake { code: 20_u8 };
    assert_eq!(
        handshake.encode().as_deref(),
        Some("X 20\r"),
        "CR only, no LF"
    );

    // 10 s = 152 * 65536 + 38528 us.
    let ten_seconds = CobraCommand::StartGate {
        edge: CobraEdge::Mode1,
        gate_us: 10_000_000_u64,
    };
    assert_eq!(ten_seconds.encode().as_deref(), Some("Z 1 152 38528\r"));

    // An exact multiple of 65536 us is sent as (hi - 1, 0): the firmware reads 0 as 65536.
    let exact = CobraCommand::StartGate {
        edge: CobraEdge::Mode2,
        gate_us: 131_072_u64,
    };
    assert_eq!(exact.encode().as_deref(), Some("Z 2 1 0\r"));

    let too_short = CobraCommand::StartGate {
        edge: CobraEdge::Mode1,
        gate_us: COBRA_MIN_GATE_US.saturating_sub(1_u64),
    };
    assert_eq!(too_short.encode(), None, "below the firmware minimum");
    let too_long = CobraCommand::StartGate {
        edge: CobraEdge::Mode1,
        gate_us: COBRA_MAX_GATE_US.saturating_add(1_u64),
    };
    assert_eq!(too_long.encode(), None, "above the firmware maximum");

    let poll = CobraCommand::PollGate {
        channel: COBRA_COUNTER_CHANNEL,
    };
    assert_eq!(poll.encode().as_deref(), Some("z 2\r"));
    let read = CobraCommand::ReadCounter { channel: 2_u8 };
    assert_eq!(read.encode().as_deref(), Some("w 2\r"));
}

#[test]
fn test_cobra_reply_parsing() {
    assert_eq!(CobraReply::parse(b"*"), CobraReply::Ack);
    assert_eq!(CobraReply::parse(b"?07"), CobraReply::Error(7_u8));
    assert_eq!(CobraReply::parse(b"X"), CobraReply::StatusX);
    assert_eq!(CobraReply::parse(b" Y "), CobraReply::StatusY);
    assert_eq!(CobraReply::parse(b"0000010A"), CobraReply::Count(266_u32));
    assert_eq!(CobraReply::parse(b"FFFFFFFF"), CobraReply::Count(u32::MAX));
    assert_eq!(
        CobraReply::parse(b"z 2"),
        CobraReply::Unknown,
        "an echoed command"
    );
    assert_eq!(
        CobraReply::parse(b"123456789"),
        CobraReply::Unknown,
        "more than 32 bits"
    );
}

#[test]
fn test_cobra_reader_assembles_cr_terminated_lines() {
    let mut reader = CobraLineReader::new();
    let replies: Vec<CobraReply> = b"*\r\n0000010A\r\nX\r"
        .iter()
        .filter_map(|&byte| reader.feed(byte))
        .collect();
    assert_eq!(
        replies,
        [
            CobraReply::Ack,
            CobraReply::Count(266_u32),
            CobraReply::StatusX
        ]
    );
}

#[test]
fn test_mcu_counter_streams_pulses_as_a_device() {
    let mut counter = McuPulseCounter::default();
    counter.on_interrupt();
    counter.on_interrupt();
    counter.on_interrupt();
    let device: &mut dyn Device = &mut counter;
    device.send(
        DeviceCommand::StartGates {
            gate_ms: 1_000_u32,
            repeat: true,
        },
        0_u64,
    );
    assert_eq!(
        device.poll(250_u64),
        Some(DeviceEvent::Pulses {
            at_ms: 250_u64,
            count: 3_u32
        })
    );
    assert_eq!(device.poll(300_u64), None, "the counter was cleared");
}
