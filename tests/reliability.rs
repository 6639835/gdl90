//! Fault recovery and public-API invariants, rather than implementation details.
use gdl90::frame::{
    FrameDecoder, crc16_ccitt, decode_frame, decode_frame_with_limit, encode_frame,
};
use gdl90::session::{
    DatagramFileWriter, RecordedDatagram, SessionWriteLimits, read_datagram_file,
    write_datagram_file,
};
use gdl90::transport::{MAX_UDP_PAYLOAD_SIZE, UdpGdl90Receiver};
use gdl90::{
    Apdu, ApduHeader, ApduReassembler, ApduSegmentation, Gdl90Error, Message, ReassemblyLimits,
    ReassemblyStatus,
};
use std::{
    fs,
    io::ErrorKind,
    net::UdpSocket,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "gdl90-reliability-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

// Deliberately independent slow recurrence: protects the optimized CRC table.
fn reference_crc(bytes: &[u8]) -> u16 {
    let mut crc = 0u16;
    for byte in bytes {
        let mut table = crc & 0xff00;
        for _ in 0..8 {
            table = if table & 0x8000 == 0 {
                table << 1
            } else {
                (table << 1) ^ 0x1021
            };
        }
        crc = table ^ (crc << 8) ^ u16::from(*byte);
    }
    crc
}
#[test]
fn crc_matches_reference_and_streaming_handles_every_split() {
    let all: Vec<_> = (0..=255).collect();
    for prefix in 0..=all.len() {
        assert_eq!(crc16_ccitt(&all[..prefix]), reference_crc(&all[..prefix]));
    }
    for first in 0..=255 {
        for second in 0..=255 {
            assert_eq!(
                crc16_ccitt(&[first, second, 0x7e]),
                reference_crc(&[first, second, 0x7e])
            );
        }
    }
    let encoded = encode_frame(&all);
    for split in 0..=encoded.len() {
        let mut decoder = FrameDecoder::new();
        let mut decoded = decoder.push(&encoded[..split]);
        decoded.extend(decoder.push(&encoded[split..]));
        assert_eq!(decoded, vec![Ok(all.clone())]);
        assert!(decoder.finish().is_none());
    }
}
#[test]
fn direct_frame_decode_requires_explicit_large_frame_policy() {
    let raw = vec![42; 2048];
    let frame = encode_frame(&raw);
    assert!(matches!(
        decode_frame(&frame),
        Err(Gdl90Error::FrameTooLong { .. })
    ));
    assert_eq!(decode_frame_with_limit(&frame, 4096).unwrap(), raw);
}

fn segment(index: u16, data: &[u8]) -> Apdu {
    Apdu {
        header: ApduHeader {
            application_flag: false,
            geo_flag: false,
            product_file_flag: false,
            product_id: 8,
            segmentation_flag: true,
            time_option: 0,
            month_day: None,
            hours: 12,
            minutes: 34,
            seconds: None,
            segmentation: Some(ApduSegmentation {
                product_file_id: 42,
                product_file_length: 2,
                apdu_number: index,
            }),
        },
        payload: data.to_vec(),
    }
}
#[test]
fn invalid_final_segment_does_not_poison_reassembly() {
    let mut reassembler = ApduReassembler::new();
    reassembler.push(segment(1, b"ABCDEFFIRST")).unwrap();
    let bytes = reassembler.buffered_bytes();
    assert!(reassembler.push(segment(2, b"BROKENLAST")).is_err());
    assert_eq!(reassembler.buffered_bytes(), bytes);
    let ReassemblyStatus::Complete(product) = reassembler.push(segment(2, b"ABCDEFLAST")).unwrap()
    else {
        panic!("not complete")
    };
    assert_eq!(product.payload, b"ABCDEFFIRSTLAST");
    assert_eq!(reassembler.buffered_bytes(), 0);
    assert_eq!(reassembler.pending_file_count(), 0);
}
#[test]
fn duplicates_cannot_pin_reassembly_past_its_hard_lifetime() {
    let start = Instant::now();
    let mut reassembler = ApduReassembler::with_limits(ReassemblyLimits {
        max_age: Duration::from_secs(2),
        ..ReassemblyLimits::default()
    })
    .unwrap();
    reassembler
        .push_from_at(1, segment(1, b"ABCDEFFIRST"), start)
        .unwrap();
    assert!(matches!(
        reassembler
            .push_from_at(
                1,
                segment(1, b"ABCDEFFIRST"),
                start + Duration::from_secs(1)
            )
            .unwrap(),
        ReassemblyStatus::Duplicate { .. }
    ));
    assert_eq!(reassembler.expire_at(start + Duration::from_secs(2)), 1);
    assert_eq!(reassembler.buffered_bytes(), 0);
}
#[test]
fn receiver_preserves_timeout_category_and_recovers_after_oversize() {
    let mut receiver = UdpGdl90Receiver::bind("127.0.0.1:0").unwrap();
    receiver.set_max_datagram_size(usize::MAX);
    assert_eq!(receiver.max_datagram_size(), MAX_UDP_PAYLOAD_SIZE);
    receiver.set_max_datagram_size(16);
    receiver
        .set_read_timeout(Some(Duration::from_millis(100)))
        .unwrap();
    let error = receiver.receive().unwrap_err();
    assert!(matches!(
        error.io_kind(),
        Some(ErrorKind::WouldBlock | ErrorKind::TimedOut)
    ));
    let sender = UdpSocket::bind("127.0.0.1:0").unwrap();
    sender
        .send_to(&[0; 128], receiver.local_addr().unwrap())
        .unwrap();
    assert!(matches!(
        receiver.receive(),
        Err(Gdl90Error::DatagramTooLarge { .. })
    ));
    let valid = encode_frame(&[4, 0, 0]);
    sender
        .send_to(&valid, receiver.local_addr().unwrap())
        .unwrap();
    let received = receiver.receive().unwrap();
    assert_eq!(received.bytes, valid);
    assert!(received.messages[0].is_ok());
}
#[test]
fn append_rejects_torn_tail_and_obeys_persistent_size_limit() {
    let dir = Directory::new();
    let path = dir.0.join("capture.txt");
    fs::write(&path, b"7E00").unwrap();
    assert!(DatagramFileWriter::open(&path, SessionWriteLimits::default()).is_err());
    assert_eq!(fs::read(&path).unwrap(), b"7E00");
    fs::remove_file(&path).unwrap();
    let data = RecordedDatagram {
        delay_ms: Some(0),
        bytes: encode_frame(&[4, 0, 0]),
    };
    let mut writer = DatagramFileWriter::open(
        &path,
        SessionWriteLimits {
            max_file_bytes: (data.to_line().len() + 1) as u64,
            ..SessionWriteLimits::default()
        },
    )
    .unwrap();
    writer.append(&data).unwrap();
    assert!(writer.append(&data).is_err());
    writer.sync_all().unwrap();
    drop(writer);
    assert_eq!(read_datagram_file(&path).unwrap(), vec![data]);
}
#[test]
fn invalid_replacement_preserves_previous_session_and_json_agrees() {
    let dir = Directory::new();
    let path = dir.0.join("session.txt");
    let data = RecordedDatagram {
        delay_ms: Some(10),
        bytes: encode_frame(&[4, 0, 0]),
    };
    write_datagram_file(&path, std::slice::from_ref(&data)).unwrap();
    let original = fs::read(&path).unwrap();
    assert!(
        write_datagram_file(
            &path,
            &[RecordedDatagram {
                delay_ms: None,
                bytes: vec![]
            }]
        )
        .is_err()
    );
    assert_eq!(fs::read(&path).unwrap(), original);
    let datagrams = [
        data,
        RecordedDatagram {
            delay_ms: None,
            bytes: vec![],
        },
    ];
    let report = gdl90::report::build_session_report(&datagrams);
    assert_eq!(
        report.analysis,
        gdl90::analysis::analyze_datagrams(&datagrams)
    );
    assert_eq!(
        report.validation,
        gdl90::analysis::validate_datagrams_syntax(&datagrams)
    );
    let json = dir.0.join("report.json");
    gdl90::report::write_json_report(&json, &report, true).unwrap();
    assert_eq!(
        fs::read_to_string(&json).unwrap(),
        gdl90::report::render_json_report(&report, true).unwrap()
    );
}
#[test]
fn invalid_public_nexrad_models_fail_without_panic() {
    use gdl90::uplink::{NexradBlock, NexradRun};
    for run in [
        NexradRun {
            count: 0,
            intensity: 0,
        },
        NexradRun {
            count: 32,
            intensity: 8,
        },
    ] {
        let block = NexradBlock::RunLengthEncoded {
            block_reference_indicator: [0x80, 0, 0],
            runs: vec![run; 4],
        };
        assert!(block.to_payload().is_err());
        assert!(block.decode_bins().is_err());
    }
    assert!(matches!(
        NexradBlock::from_payload(&[0x80, 0, 0]).unwrap(),
        NexradBlock::Unparsed { .. }
    ));
}
#[test]
fn summaries_escape_network_control_characters() {
    use gdl90::foreflight::*;
    let message = Message::ForeFlightId(ForeFlightIdMessage {
        version: 1,
        device_serial_number: None,
        device_name: "\x1b[2J".into(),
        device_long_name: "bad\nrecord".into(),
        capabilities: ForeFlightCapabilities {
            geometric_altitude_datum: GeometricAltitudeDatum::MeanSeaLevel,
            internet_policy: InternetPolicy::Disallowed,
            reserved_bits: 0,
        },
    });
    let decoded = Message::decode(&message.encode().unwrap()).unwrap();
    assert!(!decoded.summary().contains(['\x1b', '\n']));
}
#[test]
fn control_callsign_cannot_encode_a_value_its_decoder_rejects() {
    use gdl90::control::{CallSignMessage, ControlMessage};
    assert!(
        ControlMessage::CallSign(CallSignMessage {
            call_sign: "A B".into()
        })
        .encode()
        .is_err()
    );
}
