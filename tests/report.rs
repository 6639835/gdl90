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
