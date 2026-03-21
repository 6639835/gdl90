use gdl90::Gdl90Error;
use gdl90::frame::encode_frame;
use gdl90::message::{Heartbeat, HeartbeatStatus, Message};
use gdl90::report::{FrameReport, build_session_report, render_json_report, render_text_report};
use gdl90::session::RecordedDatagram;

#[test]
fn session_report_contains_frame_details() {
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
        timestamp_seconds_since_midnight: 9,
        uplink_count: 0,
        basic_and_long_count: 0,
    });

    let report = build_session_report(&[RecordedDatagram {
        delay_ms: Some(100),
        bytes: heartbeat.encode_frame().unwrap(),
    }]);

    assert_eq!(report.analysis.datagram_count, 1);
    assert_eq!(report.datagrams.len(), 1);
    assert_eq!(report.datagrams[0].frames.len(), 1);
    match &report.datagrams[0].frames[0] {
        FrameReport::Decoded { kind, summary, .. } => {
            assert_eq!(kind, "Heartbeat");
            assert!(summary.contains("gps_valid=true"));
        }
        other => panic!("expected decoded frame, got {other:?}"),
    }
}

#[test]
fn json_and_text_reports_render() {
    let report = build_session_report(&[RecordedDatagram {
        delay_ms: None,
        bytes: vec![0x01, 0x02, 0x03],
    }]);

    let text = render_text_report(&report);
    assert!(text.contains("datagrams: 1"));
    assert!(text.contains("no complete framed messages"));

    let json = render_json_report(&report, true).unwrap();
    assert!(json.contains("\"datagram_count\": 1"));
    assert!(json.contains("\"invalid_datagram_count\": 1"));
}

#[test]
fn session_report_distinguishes_decoded_message_and_frame_failures() {
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
        timestamp_seconds_since_midnight: 12,
        uplink_count: 0,
        basic_and_long_count: 0,
    });

    let decoded = heartbeat.encode_frame().unwrap();
    let message_error = encode_frame(&[0x81, 0x00]);
    let frame_error = vec![0x7E, 0x7D, 0x00, 0x7E];
    let report = build_session_report(&[RecordedDatagram {
        delay_ms: Some(25),
        bytes: [decoded, message_error, frame_error].concat(),
    }]);

    assert_eq!(report.analysis.decoded_message_count, 1);
    assert_eq!(report.analysis.decode_error_count, 2);
    assert_eq!(report.datagrams[0].frame_count, 3);

    match &report.datagrams[0].frames[0] {
        FrameReport::Decoded {
            kind,
            message_id,
            summary,
            ..
        } => {
            assert_eq!(kind, "Heartbeat");
            assert_eq!(*message_id, 0x00);
            assert!(summary.contains("utc=12"));
        }
        other => panic!("expected decoded frame, got {other:?}"),
    }

    match &report.datagrams[0].frames[1] {
        FrameReport::MessageError {
            message_id, error, ..
        } => {
            assert_eq!(*message_id, Some(0x81));
            assert_eq!(error, &Gdl90Error::InvalidMessageId(0x81).to_string());
        }
        other => panic!("expected message error, got {other:?}"),
    }

    match &report.datagrams[0].frames[2] {
        FrameReport::FrameError { error, .. } => {
            assert_eq!(error, &Gdl90Error::InvalidEscapeByte(0x00).to_string());
        }
        other => panic!("expected frame error, got {other:?}"),
    }

    let text = render_text_report(&report);
    assert!(text.contains("frame 1 kind=Heartbeat"));
    assert!(text.contains("frame 2 error: unsupported message id 0x81"));
    assert!(text.contains("frame 3 error: invalid escaped byte 0x00"));
}
