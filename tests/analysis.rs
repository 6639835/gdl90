use gdl90::analysis::{analyze_datagrams, validate_datagrams};
use gdl90::message::{Heartbeat, HeartbeatStatus, Message};
use gdl90::session::RecordedDatagram;

#[test]
fn analyzes_session_message_counts() {
    let heartbeat = Message::Heartbeat(Heartbeat {
        status: HeartbeatStatus {
            gps_position_valid: true,
            maintenance_required: false,
            ident: false,
            address_type_talkback: false,
            gps_battery_low: false,
            ratcs: false,
            uat_initialized: true,
            csa_requested: false,
            csa_not_available: false,
            utc_ok: true,
        },
        timestamp_seconds_since_midnight: 7,
        uplink_count: 0,
        basic_and_long_count: 0,
    });

    let datagrams = vec![
        RecordedDatagram {
            delay_ms: None,
            bytes: heartbeat.encode_frame().unwrap(),
        },
        RecordedDatagram {
            delay_ms: Some(250),
            bytes: heartbeat.encode_frame().unwrap(),
        },
    ];

    let analysis = analyze_datagrams(&datagrams);
    assert_eq!(analysis.datagram_count, 2);
    assert_eq!(analysis.delayed_datagram_count, 1);
    assert_eq!(analysis.total_declared_delay_ms, 250);
    assert_eq!(analysis.decoded_message_count, 2);
    assert_eq!(analysis.decode_error_count, 0);
    assert_eq!(analysis.empty_datagram_count, 0);
    assert_eq!(analysis.message_counts.get("Heartbeat"), Some(&2));
    assert!(analysis.is_clean());
}

#[test]
fn validation_reports_invalid_datagrams() {
    let datagrams = vec![
        RecordedDatagram {
            delay_ms: None,
            bytes: vec![0x01, 0x02, 0x03],
        },
        RecordedDatagram {
            delay_ms: None,
            bytes: vec![0x7E, 0x00, 0x01, 0x02, 0x7E],
        },
    ];

    let validation = validate_datagrams(&datagrams);
    assert!(!validation.is_valid());
    assert_eq!(validation.datagram_count, 2);
    assert_eq!(validation.valid_datagram_count, 0);
    assert_eq!(validation.invalid_datagram_count, 2);
    assert_eq!(validation.issues.len(), 2);
}
