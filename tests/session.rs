use std::time::{SystemTime, UNIX_EPOCH};

use gdl90::Gdl90Error;
use gdl90::message::{
    Heartbeat, HeartbeatStatus, Message, OwnshipGeometricAltitude, VerticalFigureOfMerit,
};
use gdl90::session::{
    RecordedDatagram, append_datagram, parse_datagram_line, read_datagram_file, write_datagram_file,
};

#[test]
fn recorded_datagram_decodes_messages() {
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
        timestamp_seconds_since_midnight: 42,
        uplink_count: 0,
        basic_and_long_count: 0,
    });
    let geo = Message::OwnshipGeometricAltitude(OwnshipGeometricAltitude {
        altitude_feet: 1000,
        vertical_warning: false,
        vertical_figure_of_merit: VerticalFigureOfMerit::Meters(10),
    });

    let mut bytes = heartbeat.encode_frame().unwrap();
    bytes.extend_from_slice(&geo.encode_frame().unwrap());

    let datagram = RecordedDatagram {
        delay_ms: Some(10),
        bytes,
    };
    let decoded = datagram
        .decode_messages()
        .into_iter()
        .map(Result::unwrap)
        .collect::<Vec<_>>();
    assert_eq!(decoded, vec![heartbeat, geo]);
}

#[test]
fn file_round_trip_and_append_work() {
    let path = std::env::temp_dir().join(format!(
        "gdl90-session-{}.txt",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));

    let first = RecordedDatagram {
        delay_ms: None,
        bytes: vec![0x7E, 0x00, 0x7E],
    };
    let second = RecordedDatagram {
        delay_ms: Some(250),
        bytes: vec![0x7E, 0x01, 0x7E],
    };

    write_datagram_file(&path, std::slice::from_ref(&first)).unwrap();
    append_datagram(&path, &second).unwrap();

    let records = read_datagram_file(&path).unwrap();
    assert_eq!(records, vec![first, second]);

    let _ = std::fs::remove_file(path);
}

#[test]
fn parse_rejects_invalid_lines() {
    let error = parse_datagram_line("@abc 7E00").unwrap_err();
    assert!(
        matches!(error, gdl90::Gdl90Error::InvalidField { field, .. } if field == "datagram delay")
    );
}

#[test]
fn parse_accepts_flexible_hex_delimiters_and_read_reports_line_numbers() {
    let parsed = parse_datagram_line("@15 7E:00-01 7E").unwrap().unwrap();
    assert_eq!(parsed.delay_ms, Some(15));
    assert_eq!(parsed.bytes, vec![0x7E, 0x00, 0x01, 0x7E]);

    let path = std::env::temp_dir().join(format!(
        "gdl90-invalid-session-{}.txt",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));

    std::fs::write(&path, "# comment\n@15 7E:00-01-7E\nGG\n").unwrap();
    let error = read_datagram_file(&path).unwrap_err();
    assert!(matches!(
        error,
        Gdl90Error::InvalidField { field, details }
            if field == "datagram file line"
                && details.contains("line 3")
                && details.contains("invalid hex byte")
    ));

    let _ = std::fs::remove_file(path);
}
