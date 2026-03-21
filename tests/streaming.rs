use gdl90::Gdl90Error;
use gdl90::frame::encode_frame;
use gdl90::message::{FrameMessageDecoder, Heartbeat, HeartbeatStatus, Message};

fn heartbeat_message(timestamp_seconds_since_midnight: u32) -> Message {
    Message::Heartbeat(Heartbeat {
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
        timestamp_seconds_since_midnight,
        uplink_count: 0,
        basic_and_long_count: 0,
    })
}

#[test]
fn frame_message_decoder_recovers_across_partial_corrupt_and_back_to_back_frames() {
    let first = heartbeat_message(21).encode_frame().unwrap();
    let second = heartbeat_message(22).encode_frame().unwrap();
    let mut crc_error = heartbeat_message(23).encode_frame().unwrap();
    crc_error[2] ^= 0x01;
    let unknown = encode_frame(&[0x2A, 0x10, 0x20]);
    let invalid_escape = vec![0x7E, 0x7D, 0x00, 0x7E];

    let mut decoder = FrameMessageDecoder::new();

    assert!(decoder.push(&[0x00, 0x01]).is_empty());
    assert!(decoder.push(&first[..3]).is_empty());
    let first_result = decoder.push(&first[3..]);
    assert_eq!(first_result.len(), 1);
    assert!(matches!(
        &first_result[0],
        Ok(Message::Heartbeat(Heartbeat {
            timestamp_seconds_since_midnight: 21,
            ..
        }))
    ));

    assert!(decoder.push(&invalid_escape[..2]).is_empty());
    let invalid_escape_result = decoder.push(&invalid_escape[2..]);
    assert_eq!(invalid_escape_result.len(), 1);
    assert_eq!(
        invalid_escape_result[0],
        Err(Gdl90Error::InvalidEscapeByte(0x00))
    );

    assert!(decoder.push(&crc_error[..crc_error.len() - 1]).is_empty());
    let crc_result = decoder.push(&crc_error[crc_error.len() - 1..]);
    assert_eq!(crc_result.len(), 1);
    assert!(matches!(crc_result[0], Err(Gdl90Error::CrcMismatch { .. })));

    let tail_results = decoder.push(&[unknown, second].concat());
    assert_eq!(tail_results.len(), 2);
    assert_eq!(
        tail_results[0],
        Ok(Message::Unknown {
            message_id: 0x2A,
            data: vec![0x10, 0x20],
        })
    );
    assert!(matches!(
        &tail_results[1],
        Ok(Message::Heartbeat(Heartbeat {
            timestamp_seconds_since_midnight: 22,
            ..
        }))
    ));
}
