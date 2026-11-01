//! Regression cases discovered during the September production audit.
use gdl90::analysis::{analyze_datagrams, validate_datagrams_syntax};
use gdl90::frame::{decode_frame, encode_frame};
use gdl90::message::{
    BasicUatPayload, HeightAboveTerrain, LongUatPayload, TargetReport, UatAdsbPayloadHeader,
};
use gdl90::session::RecordedDatagram;
use gdl90::transport::ForeFlightDiscoveryAnnouncement;
use std::net::{Ipv6Addr, SocketAddr, SocketAddrV6};

fn basic() -> BasicUatPayload {
    BasicUatPayload {
        header: UatAdsbPayloadHeader {
            payload_type_code: 0,
            address_qualifier: 0,
            address: 0,
        },
        state_vector: [0; 13],
        reserved: 0,
    }
}

#[test]
fn extreme_pressure_altitude_returns_error_without_overflow() {
    let mut bytes = [0; 28];
    bytes[0] = 20;
    bytes[19..27].fill(b' ');
    let mut report = TargetReport::decode(&bytes).unwrap();
    for altitude in [i32::MAX, i32::MIN, 101_351] {
        report.pressure_altitude_feet = Some(altitude);
        assert!(report.encode(20).is_err());
    }
}

#[test]
fn ground_track_uses_wide_intermediate_arithmetic() {
    let mut payload = basic();
    payload.state_vector[8] = 0x80;
    payload.state_vector[9] = 3;
    payload.state_vector[10] = 255;
    payload.state_vector[11] = 128;
    assert_eq!(payload.decoded_state_vector().track.unwrap().degrees, 359);
}

#[test]
fn ground_speed_sentinel_does_not_underflow() {
    let mut payload = basic();
    payload.state_vector[8] = 0x90;
    assert_eq!(payload.decoded_state_vector().speed_kt, None);
}

#[test]
fn declared_delay_saturates_instead_of_panicking_or_wrapping() {
    let frame = encode_frame(&[4, 0, 0]);
    let datagrams = [
        RecordedDatagram {
            delay_ms: Some(u64::MAX),
            bytes: frame.clone(),
        },
        RecordedDatagram {
            delay_ms: Some(1),
            bytes: frame,
        },
    ];
    assert_eq!(
        analyze_datagrams(&datagrams).total_declared_delay_ms,
        u64::MAX
    );
}

#[test]
fn typed_payload_encoders_reject_wrong_uat_type() {
    let mut payload = basic();
    payload.header.payload_type_code = 1;
    assert!(payload.encode().is_err());
    let long = LongUatPayload {
        header: basic().header,
        state_vector: [0; 13],
        mode_status: [0; 12],
        auxiliary_state_vector: [0; 5],
    };
    assert!(long.encode().is_err());
}

#[test]
fn typed_message_decoder_checks_identifier() {
    assert!(HeightAboveTerrain::decode(&[20, 0, 0]).is_err());
}

#[test]
fn datagram_validation_rejects_unframed_garbage() {
    let frame = encode_frame(&[4, 0, 0]);
    for garbage_first in [false, true] {
        let mut bytes = frame.clone();
        if garbage_first {
            bytes.insert(0, 42);
        } else {
            bytes.push(42);
        }
        assert!(
            !validate_datagrams_syntax(&[RecordedDatagram {
                delay_ms: None,
                bytes
            }])
            .is_valid()
        );
    }
}

#[test]
fn frame_decoder_rejects_unescaped_internal_flag() {
    let mut frame = encode_frame(&[4, 0x7e, 0]);
    let escape = frame.windows(2).position(|w| w == [0x7d, 0x5e]).unwrap();
    frame.splice(escape..escape + 2, [0x7e]);
    assert!(decode_frame(&frame).is_err());
}

#[test]
fn discovery_preserves_ipv6_scope_and_rejects_zero_port() {
    let source = SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::LOCALHOST, 63093, 12, 7));
    let announcement = ForeFlightDiscoveryAnnouncement {
        app: "ForeFlight".into(),
        gdl90_port: 4000,
    };
    let SocketAddr::V6(target) = announcement.target_for_source(source) else {
        panic!("wrong address family")
    };
    assert_eq!(target.scope_id(), 7);
    assert_eq!(target.flowinfo(), 12);
    assert_eq!(target.port(), 4000);
    assert!(
        ForeFlightDiscoveryAnnouncement::parse(r#"{"App":"ForeFlight","GDL90":{"port":0}}"#)
            .is_err()
    );
}
