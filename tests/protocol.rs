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
    AddressType, BasicUatPayload, FrameMessageDecoder, Heartbeat, HeartbeatStatus, LongUatPayload,
    Message, OwnshipGeometricAltitude, PassThroughReport, TargetAlertStatus, TargetMisc,
    TargetReport, TrackType, UatAdsbPayloadHeader, VerticalFigureOfMerit,
};
use gdl90::session::decode_hex;
use gdl90::uplink::{
    ApduHeader, FisbProduct, GenericTextApdu, GenericTextField, GenericTextRecord,
    GenericTextRecordKind, InformationFrame, NexradApdu, NexradBlock, NexradIntensity,
    TextQualifier, UatUplinkPayload,
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

    let frame = InformationFrame::from_apdu(&apdu).unwrap();
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
    .to_apdu()
    .unwrap();

    let parsed = apdu.as_nexrad().unwrap();
    assert_eq!(parsed.block.decode_bins(), bins);
    match apdu.decode_product().unwrap() {
        FisbProduct::Nexrad(_) => {}
        other => panic!("expected nexrad product, got {other:?}"),
    }
}

#[test]
fn apdu_rejects_unsupported_optional_or_segmented_headers() {
    let error = gdl90::uplink::Apdu::from_bytes(&[0x80, 0x00, 0x00, 0x00]).unwrap_err();
    assert!(
        matches!(error, gdl90::Gdl90Error::InvalidField { field, .. } if field == "APDU product descriptor options")
    );

    let header = ApduHeader {
        application_flag: false,
        geo_flag: false,
        product_file_flag: false,
        product_id: 63,
        segmentation_flag: true,
        time_option: 0,
        hours: 0,
        minutes: 0,
    };
    let bytes = header.to_bytes().unwrap();
    let error = gdl90::uplink::Apdu::from_bytes(&bytes).unwrap_err();
    assert!(
        matches!(error, gdl90::Gdl90Error::InvalidField { field, .. } if field == "APDU segmentation")
    );
}

#[test]
fn generic_text_and_nexrad_validate_documented_minimal_headers() {
    let generic_error = GenericTextApdu {
        header: ApduHeader {
            application_flag: false,
            geo_flag: false,
            product_file_flag: false,
            product_id: 413,
            segmentation_flag: false,
            time_option: 1,
            hours: 0,
            minutes: 0,
        },
        records: vec![GenericTextRecord {
            kind: GenericTextRecordKind::Metar,
            record_type: "METAR".to_string(),
            location: GenericTextField::Text("KSFO".to_string()),
            record_time: GenericTextField::Text("260900Z".to_string()),
            qualifier: None,
            text: "AUTO 28012KT 10SM CLR=".to_string(),
        }],
    }
    .to_apdu()
    .unwrap_err();
    assert!(
        matches!(generic_error, gdl90::Gdl90Error::InvalidField { field, .. } if field == "APDU time option")
    );

    let nexrad_error = NexradApdu {
        header: ApduHeader {
            application_flag: true,
            geo_flag: false,
            product_file_flag: false,
            product_id: 63,
            segmentation_flag: false,
            time_option: 0,
            hours: 0,
            minutes: 0,
        },
        block: NexradBlock::Empty {
            block_reference_indicator: [0x84, 0xA5, 0x70],
        },
    }
    .to_apdu()
    .unwrap_err();
    assert!(
        matches!(nexrad_error, gdl90::Gdl90Error::InvalidField { field, .. } if field == "APDU product descriptor options")
    );
}

#[test]
fn generic_text_pack_records_keeps_whole_records_within_apdu_limit() {
    let header = ApduHeader {
        application_flag: false,
        geo_flag: false,
        product_file_flag: false,
        product_id: 413,
        segmentation_flag: false,
        time_option: 0,
        hours: 0,
        minutes: 0,
    };
    let make_record = |location: &str, text: &str| GenericTextRecord {
        kind: GenericTextRecordKind::Taf,
        record_type: "TAF".to_string(),
        location: GenericTextField::Text(location.to_string()),
        record_time: GenericTextField::Text("260900Z".to_string()),
        qualifier: None,
        text: text.to_string(),
    };

    let records = vec![
        make_record("KSFO", &"A".repeat(180)),
        make_record("KOAK", &"B".repeat(180)),
        make_record("KSQL", &"C".repeat(180)),
    ];

    let apdus = GenericTextApdu::pack_records(header, &records).unwrap();
    assert_eq!(apdus.len(), 2);
    assert_eq!(apdus[0].records.len(), 2);
    assert_eq!(apdus[1].records.len(), 1);

    for apdu in apdus {
        let encoded = apdu.to_apdu().unwrap();
        assert!(encoded.payload.len() <= 418);
    }
}

#[test]
fn nexrad_intensity_rows_expose_table_20_semantics() {
    let mut bins = [0u8; 128];
    bins[0] = 0;
    bins[1] = 1;
    bins[2] = 2;
    bins[3] = 7;
    let block = NexradBlock::from_bins([0x84, 0xA5, 0x70], &bins).unwrap();

    let rows = block.decode_intensity_rows().unwrap();
    assert_eq!(rows.len(), 4);
    assert_eq!(rows[0][0], NexradIntensity::Value0);
    assert_eq!(rows[0][1], NexradIntensity::Value1);
    assert_eq!(rows[0][2].weather_condition(), "VIP 1");
    assert_eq!(rows[0][3].weather_condition(), "VIP 6");
    assert_eq!(rows[0][0].reflectivity_range(), "dBz < 5");
    assert!(rows[0][1].is_background());
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
fn basic_pass_through_report_exposes_inner_payload_sections() {
    let payload = BasicUatPayload {
        header: UatAdsbPayloadHeader {
            payload_type_code: 0,
            address_qualifier: 2,
            address: 0xAB_CD_EF,
        },
        state_vector: [0x11; 13],
        reserved: 0x7A,
    };
    let report = PassThroughReport::<18>::from_basic_payload(Some(0x12_34_56), &payload).unwrap();
    let message = Message::BasicReport(report.clone());

    let decoded = match Message::decode(&message.encode().unwrap()).unwrap() {
        Message::BasicReport(report) => report,
        other => panic!("expected basic report, got {other:?}"),
    };

    assert_eq!(decoded.time_of_reception, Some(0x12_34_56));
    assert_eq!(decoded.basic_payload(), payload);
    assert_eq!(decoded.basic_payload().encode().unwrap(), report.payload);
}

#[test]
fn long_pass_through_report_exposes_inner_payload_sections() {
    let payload = LongUatPayload {
        header: UatAdsbPayloadHeader {
            payload_type_code: 1,
            address_qualifier: 0,
            address: 0x01_23_45,
        },
        state_vector: [0x22; 13],
        mode_status: [0x33; 12],
        auxiliary_state_vector: [0x44; 5],
    };
    let report = PassThroughReport::<34>::from_long_payload(None, &payload).unwrap();
    let message = Message::LongReport(report.clone());

    let decoded = match Message::decode(&message.encode().unwrap()).unwrap() {
        Message::LongReport(report) => report,
        other => panic!("expected long report, got {other:?}"),
    };

    assert_eq!(decoded.time_of_reception, None);
    assert_eq!(decoded.long_payload(), payload);
    assert_eq!(decoded.long_payload().encode().unwrap(), report.payload);
}

#[test]
fn generic_text_nil_fields_and_qualifier_rules_are_supported() {
    let taf = GenericTextRecord::parse("TAF NIL= 260900Z AM TEST REPORT").unwrap();
    assert_eq!(taf.kind, GenericTextRecordKind::Taf);
    assert_eq!(taf.location, GenericTextField::Nil);
    assert_eq!(
        taf.record_time,
        GenericTextField::Text("260900Z".to_string())
    );
    taf.validate_metar_taf_composition().unwrap();

    let metar = GenericTextRecord::parse("METAR KSLE NIL= SP TEST REPORT").unwrap();
    assert_eq!(metar.kind, GenericTextRecordKind::Metar);
    assert_eq!(metar.location, GenericTextField::Text("KSLE".to_string()));
    assert_eq!(metar.record_time, GenericTextField::Nil);
    metar.validate_metar_taf_composition().unwrap();

    let invalid = GenericTextRecord::parse("METAR KSLE 260900Z AM TEST REPORT").unwrap();
    assert!(invalid.validate_metar_taf_composition().is_err());
}

#[test]
fn sample_text_application_data_field_decodes_to_five_tafs() {
    let hex = concat!(
        "2180067441905011a02d3305832db0e70c1a04d832d71cf1d60c38c30d8b5204364cd806157c36c",
        "2008b3b1cb079c146370d30c205920b0ccb5204364cd8130d4cb5c3d79d2180067441905011a02d",
        "0118832db0e70c1a04d832d71cf1d60c38c30d8b5204364cd806157c36c2008b3b1cb079c146370",
        "d30c205920b0ccb5204364cd8130d4cb5c3d79d2180067441905011a02c5547832db0e70c1a04d8",
        "32d71cf1d60c38c30d8b5204364cd806157c36c2008b3b1cb079c146370d30c205920b0ccb52043",
        "64cd8130d4cb5c3d79d2180067441905011a02c14d4832db0e70c1a04d832d71cf1d60c38c30d8b",
        "5204364cd806157c36c2008b3b1cb079c146370d30c205920b0ccb5204364cd8130d4cb5c3d79d2",
        "180067441905011a02c824f832db0e70c1a04d832d71cf1d60c38c30d8b5204364cd806157c36c2",
        "008b3b1cb079c146370d30c205920b0ccb5204364cd8130d4cb5c3d79d00000000000000000000",
        "000000000000000000000000000000000000000000000000000000000000000000000000000000",
        "000000000000000000000000000000000000000000000000000000000000"
    );
    let bytes = decode_hex(hex).unwrap();
    assert_eq!(bytes.len(), 424);
    let payload = UatUplinkPayload {
        header: [0u8; 8],
        application_data: bytes.try_into().unwrap(),
    };

    let products = payload.fisb_products().unwrap();
    assert_eq!(products.len(), 5);
    for product in products {
        match product {
            FisbProduct::GenericText(text) => {
                text.validate_records().unwrap();
                assert_eq!(text.records.len(), 1);
                assert_eq!(text.records[0].kind, GenericTextRecordKind::Taf);
            }
            other => panic!("expected generic text product, got {other:?}"),
        }
    }
}

#[test]
fn sample_nexrad_application_data_fields_decode_to_nineteen_products() {
    let field1 = concat!(
        "130000FC000084A570308950111A53120930110A23451B0A0918090A1B0C1D0607061D041",
        "B0A0108208000FC000084A3AE00090A1314150617061D04130A01080112131C0D06270615",
        "140B0A01000112131C0D06270615140B0A010000090A1314150617061D04130A0108148000",
        "FC000084A1EC00090A1B0C1D0607061D041B0A010808110A23451B0A091018111A531209",
        "20308930130000FC000084AAB7308950111A53120930110A23451B0A0918090A1B0C1D060",
        "7061D041B0A0108208000FC000084A8F500090A1314150617061D04130A01080112131C0D",
        "06270615140B0A01000112131C0D06270615140B0A010000090A1314150617061D04130A01",
        "08148000FC000084A73300090A1B0C1D0607061D041B0A010808110A23451B0A091018111",
        "A53120920308930130000FC000084AFFD308950111A53120930110A23451B0A0918090A1B",
        "0C1D0607061D041B0A010800000000000000000000000000000000000000000000000000000",
        "000000000000000000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000"
    );
    let field2 = concat!(
        "208000FC000084AE3B00090A1314150617061D04130A01080112131C0D06270615140B0A0",
        "1000112131C0D06270615140B0A010000090A1314150617061D04130A0108148000FC00008",
        "4AC7900090A1B0C1D0607061D041B0A010808110A23451B0A091018111A5312092030893",
        "0040000FC000004B1BDF0040000FC000004AFFBD0040000FC000004AE39D0040000FC000",
        "004AC77D0040000FC000004AAB5D0040000FC000004A8F3D0040000FC000004A731D004",
        "0000FC000004A56FE0040000FC000004A3ADE0040000FC000004A1EBE0000000000000000",
        "000000000000000000000000000000000000000000000000000000000000000000000000000000",
        "000000000000000000000000000000000000000000000000000000000000000000000000000000",
        "000000000000000000000000000000000000000000000000000000000000000000000000000000",
        "000000000000000000000000000000000000000000000000000000000000000000000000000000",
        "000000000000000000000000000000000000000000000000000000000000000000000000000000",
        "00000000000000000000000"
    );

    let payloads = [field1, field2]
        .into_iter()
        .map(|hex| UatUplinkPayload {
            header: [0u8; 8],
            application_data: decode_hex(hex).unwrap().try_into().unwrap(),
        })
        .collect::<Vec<_>>();

    let mut products = Vec::new();
    for payload in payloads {
        products.extend(payload.fisb_products().unwrap());
    }

    assert_eq!(products.len(), 19);
    let mut nexrad_count = 0usize;
    let mut empty_or_unparsed_count = 0usize;
    let mut rle_count = 0usize;
    for product in products {
        match product {
            FisbProduct::Nexrad(nexrad) => {
                nexrad_count += 1;
                match nexrad.block {
                    NexradBlock::Empty { .. } | NexradBlock::Unparsed { .. } => {
                        empty_or_unparsed_count += 1
                    }
                    NexradBlock::RunLengthEncoded { .. } => rle_count += 1,
                }
            }
            other => panic!("expected nexrad product, got {other:?}"),
        }
    }

    assert_eq!(nexrad_count, 19);
    assert_eq!(empty_or_unparsed_count, 10);
    assert_eq!(rle_count, 9);
}
