use gdl90::control::{
    CallSignMessage, ControlMessage, ControlMode, EmergencyCode, IdentStatus, ModeMessage,
    VfrCodeMessage,
};
use gdl90::foreflight::{
    ForeFlightAhrsMessage, ForeFlightCapabilities, ForeFlightIdMessage, GeometricAltitudeDatum,
    Heading, HeadingType, InternetPolicy,
};
use gdl90::frame::decode_frame;
use gdl90::message::{
    AddressType, FrameMessageDecoder, Heartbeat, HeartbeatStatus, Message,
    OwnshipGeometricAltitude, TargetAlertStatus, TargetMisc, TargetReport, TrackType,
    VerticalFigureOfMerit,
};
use gdl90::uplink::{
    ApduHeader, FisbProduct, GenericTextApdu, GenericTextField, GenericTextRecord,
    GenericTextRecordKind, InformationFrame, NexradApdu, NexradBlock, TextQualifier,
    UatUplinkPayload,
};

#[test]
fn heartbeat_spec_frame_decodes_and_reencodes() {
    let frame = [
        0x7E, 0x00, 0x81, 0x41, 0xDB, 0xD0, 0x08, 0x02, 0xB3, 0x8B, 0x7E,
    ];
    let clear = decode_frame(&frame).unwrap();
    let message = Message::decode(&clear).unwrap();

    match message {
        Message::Heartbeat(heartbeat) => {
            assert!(heartbeat.status.gps_position_valid);
            assert!(heartbeat.status.uat_initialized);
            assert!(heartbeat.status.utc_ok);
            assert_eq!(heartbeat.timestamp_seconds_since_midnight, 53_467);
            assert_eq!(heartbeat.uplink_count, 1);
            assert_eq!(heartbeat.basic_and_long_count, 2);
        }
        other => panic!("expected heartbeat, got {other:?}"),
    }

    assert_eq!(gdl90::frame::encode_frame(&clear), frame);
}

#[test]
fn traffic_report_spec_example_round_trips() {
    let bytes = [
        0x14, 0x00, 0xAB, 0x45, 0x49, 0x1F, 0xEF, 0x15, 0xA8, 0x89, 0x78, 0x0F, 0x09, 0xA9, 0x07,
        0xB0, 0x01, 0x20, 0x01, 0x4E, 0x38, 0x32, 0x35, 0x56, 0x20, 0x20, 0x20, 0x00,
    ];
    let report = match Message::decode(&bytes).unwrap() {
        Message::TrafficReport(report) => report,
        other => panic!("expected traffic report, got {other:?}"),
    };

    assert_eq!(report.alert_status, TargetAlertStatus::NoAlert);
    assert_eq!(report.address_type, AddressType::AdsbIcao);
    assert_eq!(report.participant_address, 0xAB4549);
    assert!((report.latitude_degrees - 44.90708).abs() < 0.001);
    assert!((report.longitude_degrees - (-122.99488)).abs() < 0.001);
    assert_eq!(report.pressure_altitude_feet, Some(5_000));
    assert!(report.misc.airborne);
    assert!(!report.misc.extrapolated);
    assert_eq!(report.misc.track_type, TrackType::TrueTrack);
    assert_eq!(report.nic, 10);
    assert_eq!(report.nacp, 9);
    assert_eq!(report.horizontal_velocity_knots, Some(123));
    assert_eq!(report.vertical_velocity_fpm, Some(64));
    assert_eq!(report.track_heading, Some(0x20));
    assert_eq!(report.emitter_category, 1);
    assert_eq!(report.call_sign, "N825V");
    assert_eq!(report.emergency_priority_code, 0);

    let reencoded = Message::TrafficReport(report).encode().unwrap();
    assert_eq!(reencoded, bytes);
}

#[test]
fn framed_stream_decoder_handles_back_to_back_messages() {
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
        timestamp_seconds_since_midnight: 5,
        uplink_count: 0,
        basic_and_long_count: 3,
    });

    let geo = Message::OwnshipGeometricAltitude(OwnshipGeometricAltitude {
        altitude_feet: 1_000,
        vertical_warning: true,
        vertical_figure_of_merit: VerticalFigureOfMerit::Meters(50),
    });

    let mut stream = heartbeat.encode_frame().unwrap();
    stream.extend_from_slice(&geo.encode_frame().unwrap());

    let mut decoder = FrameMessageDecoder::new();
    let messages = decoder
        .push(&stream)
        .into_iter()
        .map(Result::unwrap)
        .collect::<Vec<_>>();

    assert_eq!(messages, vec![heartbeat, geo]);
}

#[test]
fn foreflight_messages_round_trip() {
    let id = ForeFlightIdMessage {
        version: 1,
        device_serial_number: Some(0x1122_3344_5566_7788),
        device_name: "GDL90".to_string(),
        device_long_name: "Rust ForeFlight".to_string(),
        capabilities: ForeFlightCapabilities {
            geometric_altitude_datum: GeometricAltitudeDatum::MeanSeaLevel,
            internet_policy: InternetPolicy::Expensive,
            reserved_bits: 0,
        },
    };
    let ahrs = ForeFlightAhrsMessage {
        roll_tenths_degrees: Some(100),
        pitch_tenths_degrees: Some(-50),
        heading: Some(Heading {
            heading_type: HeadingType::Magnetic,
            tenths_degrees: 900,
        }),
        indicated_airspeed_knots: Some(120),
        true_airspeed_knots: None,
    };

    let id_message = Message::ForeFlightId(id.clone());
    let ahrs_message = Message::ForeFlightAhrs(ahrs.clone());
    assert_eq!(
        Message::decode(&id_message.encode().unwrap()).unwrap(),
        id_message
    );
    assert_eq!(
        Message::decode(&ahrs_message.encode().unwrap()).unwrap(),
        ahrs_message
    );
}

#[test]
fn control_messages_round_trip() {
    let mode = ControlMessage::Mode(ModeMessage {
        mode: ControlMode::ModeA,
        ident: IdentStatus::Active,
        squawk: "2354".to_string(),
        emergency: EmergencyCode::None,
        healthy: true,
    });
    let encoded_mode = mode.encode().unwrap();
    assert_eq!(&encoded_mode, b"^MD A,I,23540120\r");
    assert_eq!(ControlMessage::decode(&encoded_mode).unwrap(), mode);

    let call_sign = ControlMessage::CallSign(CallSignMessage {
        call_sign: "GARMIN".to_string(),
    });
    assert_eq!(
        ControlMessage::decode(&call_sign.encode().unwrap()).unwrap(),
        call_sign
    );

    let vfr = ControlMessage::VfrCode(VfrCodeMessage {
        vfr_code: "1200".to_string(),
    });
    assert_eq!(ControlMessage::decode(&vfr.encode().unwrap()).unwrap(), vfr);
}

#[test]
fn generic_text_uplink_round_trip() {
    let record = GenericTextRecord {
        kind: GenericTextRecordKind::Taf,
        record_type: "TAF".to_string(),
        location: GenericTextField::Text("KSLE".to_string()),
        record_time: GenericTextField::Text("260900Z".to_string()),
        qualifier: Some(TextQualifier::Amendment),
        text: "251315 08006KT P6SM FEW060 BKN120".to_string(),
    };
    record.validate_metar_taf_composition().unwrap();
    let apdu = GenericTextApdu {
        header: ApduHeader {
            application_flag: false,
            geo_flag: false,
            product_file_flag: false,
            product_id: 413,
            segmentation_flag: false,
            time_option: 0,
            hours: 16,
            minutes: 25,
        },
        records: vec![record.clone()],
    }
    .to_apdu()
    .unwrap();

    let frame = InformationFrame::from_apdu(&apdu);
    let payload = UatUplinkPayload::from_information_frames([0u8; 8], &[frame]).unwrap();
    let frames = payload.information_frames().unwrap();
    let parsed_apdu = frames[0].apdu().unwrap();
    match parsed_apdu.decode_product().unwrap() {
        FisbProduct::GenericText(_) => {}
        other => panic!("expected generic text product, got {other:?}"),
    }
    let parsed_text = parsed_apdu.as_generic_text().unwrap();

    assert_eq!(parsed_text.records, vec![record]);
}

#[test]
fn nexrad_rle_block_round_trip() {
    let mut bins = [0u8; 128];
    bins[10..20].fill(1);
    bins[40..48].fill(3);
    bins[90..96].fill(7);

    let block = NexradBlock::from_bins([0x84, 0xA5, 0x70], &bins).unwrap();
    let apdu = NexradApdu {
        header: ApduHeader {
            application_flag: false,
            geo_flag: false,
            product_file_flag: false,
            product_id: 63,
            segmentation_flag: false,
            time_option: 0,
            hours: 0,
            minutes: 0,
        },
        block,
    }
    .to_apdu();

    let parsed = apdu.as_nexrad().unwrap();
    assert_eq!(parsed.block.decode_bins(), bins);
    match apdu.decode_product().unwrap() {
        FisbProduct::Nexrad(_) => {}
        other => panic!("expected nexrad product, got {other:?}"),
    }
}

#[test]
fn ownship_report_round_trip() {
    let report = TargetReport {
        alert_status: TargetAlertStatus::NoAlert,
        address_type: AddressType::AdsbSelfAssigned,
        participant_address: 0x00_00_01,
        latitude_degrees: 37.6188056,
        longitude_degrees: -122.3754167,
        pressure_altitude_feet: Some(150),
        misc: TargetMisc {
            airborne: true,
            extrapolated: false,
            track_type: TrackType::TrueTrack,
        },
        nic: 9,
        nacp: 10,
        horizontal_velocity_knots: Some(120),
        vertical_velocity_fpm: Some(0),
        track_heading: Some(32),
        emitter_category: 1,
        call_sign: "N12345".to_string(),
        emergency_priority_code: 0,
        spare: 0,
    };

    let encoded = Message::OwnshipReport(report.clone()).encode().unwrap();
    let decoded = Message::decode(&encoded).unwrap();
    let Message::OwnshipReport(decoded) = decoded else {
        panic!("expected ownship report");
    };
    assert_eq!(decoded.alert_status, report.alert_status);
    assert_eq!(decoded.address_type, report.address_type);
    assert_eq!(decoded.participant_address, report.participant_address);
    assert!((decoded.latitude_degrees - report.latitude_degrees).abs() < 0.00002);
    assert!((decoded.longitude_degrees - report.longitude_degrees).abs() < 0.00002);
    assert_eq!(
        decoded.pressure_altitude_feet,
        report.pressure_altitude_feet
    );
    assert_eq!(decoded.misc, report.misc);
    assert_eq!(decoded.nic, report.nic);
    assert_eq!(decoded.nacp, report.nacp);
    assert_eq!(
        decoded.horizontal_velocity_knots,
        report.horizontal_velocity_knots
    );
    assert_eq!(decoded.vertical_velocity_fpm, report.vertical_velocity_fpm);
    assert_eq!(decoded.track_heading, report.track_heading);
    assert_eq!(decoded.emitter_category, report.emitter_category);
    assert_eq!(decoded.call_sign, report.call_sign);
    assert_eq!(
        decoded.emergency_priority_code,
        report.emergency_priority_code
    );
    assert_eq!(decoded.spare, report.spare);
}

#[test]
fn generic_text_nil_fields_and_qualifier_rules_are_supported() {
    let taf = GenericTextRecord::parse("TAF NIL= 260900Z AM TEST REPORT").unwrap();
    assert_eq!(taf.kind, GenericTextRecordKind::Taf);
    assert_eq!(taf.location, GenericTextField::Nil);
    assert_eq!(taf.record_time, GenericTextField::Text("260900Z".to_string()));
    taf.validate_metar_taf_composition().unwrap();

    let metar = GenericTextRecord::parse("METAR KSLE NIL= SP TEST REPORT").unwrap();
    assert_eq!(metar.kind, GenericTextRecordKind::Metar);
    assert_eq!(metar.location, GenericTextField::Text("KSLE".to_string()));
    assert_eq!(metar.record_time, GenericTextField::Nil);
    metar.validate_metar_taf_composition().unwrap();

    let invalid = GenericTextRecord::parse("METAR KSLE 260900Z AM TEST REPORT").unwrap();
    assert!(invalid.validate_metar_taf_composition().is_err());
}
