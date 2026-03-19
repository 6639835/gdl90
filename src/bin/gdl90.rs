use std::env;
use std::fs;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use gdl90::analysis::{analyze_datagrams, validate_datagrams};
use gdl90::foreflight::{
    ForeFlightAhrsMessage, ForeFlightCapabilities, ForeFlightIdMessage, GeometricAltitudeDatum,
    Heading, HeadingType, InternetPolicy,
};
use gdl90::message::{
    AddressType, Heartbeat, HeartbeatStatus, Message, OwnshipGeometricAltitude, TargetAlertStatus,
    TargetMisc, TargetReport, TrackType, VerticalFigureOfMerit,
};
use gdl90::report::{build_session_report, render_json_report, render_text_report};
use gdl90::session::{RecordedDatagram, append_datagram, decode_hex, read_datagram_file};
use gdl90::support::{
    SupportState, control_panel_connections, control_panel_profiles, missing_sections,
    rs422_bus_profile, rs422_connections, section_support_matrix,
};
use gdl90::transport::{
    FOREFLIGHT_DISCOVERY_PORT, FOREFLIGHT_GDL90_PORT, ForeFlightUdpSender, UdpGdl90Receiver,
    UdpGdl90Sender, discover_foreflight_once,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("decode-frame") => {
            let hex = require_arg(args.next(), "hex frame")?;
            let bytes = decode_hex(&hex).map_err(boxed_string_error)?;
            let clear = gdl90::frame::decode_frame(&bytes)?;
            let message = Message::decode(&clear)?;
            println!("{message:#?}");
        }
        Some("decode-stream") => {
            let hex = require_arg(args.next(), "hex stream")?;
            let bytes = decode_hex(&hex).map_err(boxed_string_error)?;
            let mut decoder = gdl90::FrameMessageDecoder::new();
            for result in decoder.push(&bytes) {
                println!("{:#?}", result?);
            }
        }
        Some("decode-file") => {
            let path = PathBuf::from(require_arg(args.next(), "session file")?);
            let datagrams = read_datagram_file(&path)?;
            for (index, datagram) in datagrams.iter().enumerate() {
                println!(
                    "datagram {} delay={:?} bytes={}",
                    index + 1,
                    datagram.delay_ms,
                    datagram.bytes.len()
                );
                for message in datagram.decode_messages() {
                    match message {
                        Ok(message) => println!("{message:#?}"),
                        Err(error) => println!("decode error: {error}"),
                    }
                }
            }
        }
        Some("report-file") => {
            let path = PathBuf::from(require_arg(args.next(), "session file")?);
            let datagrams = read_datagram_file(&path)?;
            let report = build_session_report(&datagrams);
            print!("{}", render_text_report(&report));
        }
        Some("report-file-json") => {
            let path = PathBuf::from(require_arg(args.next(), "session file")?);
            let output = args.next().map(PathBuf::from);
            let datagrams = read_datagram_file(&path)?;
            let report = build_session_report(&datagrams);
            let json = render_json_report(&report, true)?;
            if let Some(output) = output {
                fs::write(&output, json)?;
                println!("wrote {}", output.display());
            } else {
                println!("{json}");
            }
        }
        Some("support-status") => {
            let only_missing = matches!(args.next().as_deref(), Some("--missing"));
            let entries = if only_missing {
                missing_sections()
            } else {
                section_support_matrix()
            };
            for entry in entries {
                println!(
                    "{} [{}] {}",
                    entry.section,
                    render_support_state(entry.state),
                    entry.title
                );
                println!("  {}", entry.notes);
            }
        }
        Some("interface-profiles") => {
            let rs422 = rs422_bus_profile();
            println!(
                "RS-422 bus: {:?} {} baud {}{}{} {:?} {:?}",
                rs422.signal_type,
                rs422.baud_rate,
                rs422.start_bits,
                rs422.data_bits,
                rs422.stop_bits,
                rs422.parity,
                rs422.flow_control
            );
            for connection in rs422_connections() {
                println!(
                    "  {} | {} | {}",
                    connection.signal_name, connection.direction, connection.connector_pin
                );
            }
            println!("Control panel profiles:");
            for profile in control_panel_profiles() {
                println!(
                    "  {:?} {} baud {}{}{} {:?} {:?}",
                    profile.signal_type,
                    profile.baud_rate,
                    profile.start_bits,
                    profile.data_bits,
                    profile.stop_bits,
                    profile.parity,
                    profile.flow_control
                );
            }
            for connection in control_panel_connections() {
                println!(
                    "  {} | {} | {}",
                    connection.signal_name, connection.direction, connection.connector_pin
                );
            }
        }
        Some("analyze-file") => {
            let path = PathBuf::from(require_arg(args.next(), "session file")?);
            let datagrams = read_datagram_file(&path)?;
            let analysis = analyze_datagrams(&datagrams);
            println!("datagrams: {}", analysis.datagram_count);
            println!("total bytes: {}", analysis.total_bytes);
            println!("delayed datagrams: {}", analysis.delayed_datagram_count);
            println!(
                "declared replay delay ms: {}",
                analysis.total_declared_delay_ms
            );
            println!("decoded messages: {}", analysis.decoded_message_count);
            println!("decode errors: {}", analysis.decode_error_count);
            println!("empty datagrams: {}", analysis.empty_datagram_count);
            println!(
                "max messages per datagram: {}",
                analysis.max_messages_per_datagram
            );
            println!("message counts:");
            for (kind, count) in analysis.message_counts {
                println!("  {kind}: {count}");
            }
        }
        Some("validate-file") => {
            let path = PathBuf::from(require_arg(args.next(), "session file")?);
            let datagrams = read_datagram_file(&path)?;
            let validation = validate_datagrams(&datagrams);
            if validation.is_valid() {
                println!(
                    "valid: {} datagrams, no decode issues",
                    validation.datagram_count
                );
            } else {
                println!(
                    "invalid: {} of {} datagrams have issues",
                    validation.invalid_datagram_count, validation.datagram_count
                );
                for issue in validation.issues {
                    println!("  datagram {}: {}", issue.datagram_index, issue.details);
                }
                return Err("session validation failed".into());
            }
        }
        Some("listen") => {
            let bind = args
                .next()
                .unwrap_or_else(|| format!("0.0.0.0:{FOREFLIGHT_GDL90_PORT}"));
            let mut receiver = UdpGdl90Receiver::bind(&bind)?;
            println!("listening on {}", receiver.local_addr()?);
            loop {
                let datagram = receiver.receive()?;
                println!("from {} ({} bytes)", datagram.source, datagram.bytes.len());
                for message in datagram.messages {
                    match message {
                        Ok(message) => println!("{message:#?}"),
                        Err(error) => println!("decode error: {error}"),
                    }
                }
            }
        }
        Some("discover") => {
            let bind = args
                .next()
                .unwrap_or_else(|| format!("0.0.0.0:{FOREFLIGHT_DISCOVERY_PORT}"));
            let timeout_secs = args
                .next()
                .map(|value| value.parse::<u64>())
                .transpose()?
                .unwrap_or(10);
            let (source, announcement) =
                discover_foreflight_once(&bind, Duration::from_secs(timeout_secs))?;
            println!("source: {source}");
            println!("announcement: {announcement:#?}");
            if announcement.is_foreflight() {
                println!(
                    "suggested target: {}",
                    announcement.target_for_source(source)
                );
            }
        }
        Some("capture") => {
            let bind = args
                .next()
                .unwrap_or_else(|| format!("0.0.0.0:{FOREFLIGHT_GDL90_PORT}"));
            let output = PathBuf::from(require_arg(args.next(), "output file")?);
            let count = args
                .next()
                .map(|value| value.parse::<usize>())
                .transpose()?
                .unwrap_or(0);

            let mut receiver = UdpGdl90Receiver::bind(&bind)?;
            println!(
                "capturing on {} to {}",
                receiver.local_addr()?,
                output.display()
            );
            let mut seen = 0usize;
            loop {
                let datagram = receiver.receive()?;
                append_datagram(
                    &output,
                    &RecordedDatagram {
                        delay_ms: None,
                        bytes: datagram.bytes,
                    },
                )?;
                seen += 1;
                println!("captured datagram {seen} from {}", datagram.source);
                if count != 0 && seen >= count {
                    break;
                }
            }
        }
        Some("send-demo") => {
            let target = require_arg(args.next(), "target host:port")?;
            let count = args
                .next()
                .map(|value| value.parse::<u32>())
                .transpose()?
                .unwrap_or(5);
            let interval_ms = args
                .next()
                .map(|value| value.parse::<u64>())
                .transpose()?
                .unwrap_or(1_000);
            let sender = ForeFlightUdpSender::bind("0.0.0.0:0", &target)?;
            println!(
                "sending demo traffic from {} to {}",
                sender.local_addr()?,
                target
            );
            for tick in 0..count {
                let messages = demo_messages(tick);
                sender.send_messages(&messages)?;
                thread::sleep(Duration::from_millis(interval_ms));
            }
        }
        Some("replay-file") => {
            let path = PathBuf::from(require_arg(args.next(), "session file")?);
            let target = require_arg(args.next(), "target host:port")?;
            let default_interval_ms = args
                .next()
                .map(|value| value.parse::<u64>())
                .transpose()?
                .unwrap_or(0);
            let datagrams = read_datagram_file(&path)?;
            let sender = UdpGdl90Sender::bind("0.0.0.0:0", &target)?;
            println!(
                "replaying {} datagrams from {} to {}",
                datagrams.len(),
                path.display(),
                target
            );
            let mut first = true;
            for datagram in datagrams {
                let delay_ms = if first {
                    datagram.delay_ms.unwrap_or(0)
                } else {
                    datagram.delay_ms.unwrap_or(default_interval_ms)
                };
                if delay_ms != 0 {
                    thread::sleep(Duration::from_millis(delay_ms));
                }
                sender.send_frame(&datagram.bytes)?;
                first = false;
            }
        }
        _ => print_usage(),
    }

    Ok(())
}

fn require_arg(value: Option<String>, name: &'static str) -> Result<String, String> {
    value.ok_or_else(|| format!("missing required argument: {name}"))
}

fn boxed_string_error(error: String) -> Box<dyn std::error::Error> {
    error.into()
}

fn render_support_state(state: SupportState) -> &'static str {
    match state {
        SupportState::Complete => "complete",
        SupportState::Partial => "partial",
        SupportState::NotImplemented => "not-implemented",
        SupportState::BlockedByExternalSpec => "blocked-by-external-spec",
        SupportState::OutOfScopeBehavior => "out-of-scope-behavior",
    }
}

fn demo_messages(tick: u32) -> Vec<Message> {
    let timestamp = tick % 86_400;
    let lat = 37.6188056 + (tick as f64 * 0.0001);
    let lon = -122.3754167 + (tick as f64 * 0.0001);

    vec![
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
            timestamp_seconds_since_midnight: timestamp,
            uplink_count: 0,
            basic_and_long_count: 0,
        }),
        Message::OwnshipReport(TargetReport {
            alert_status: TargetAlertStatus::NoAlert,
            address_type: AddressType::AdsbSelfAssigned,
            participant_address: 0xF0_00_00,
            latitude_degrees: lat,
            longitude_degrees: lon,
            pressure_altitude_feet: Some(1_500),
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
        }),
        Message::OwnshipGeometricAltitude(OwnshipGeometricAltitude {
            altitude_feet: 1_525,
            vertical_warning: false,
            vertical_figure_of_merit: VerticalFigureOfMerit::Meters(8),
        }),
        Message::ForeFlightId(ForeFlightIdMessage {
            version: 1,
            device_serial_number: Some(42),
            device_name: "GDL90".to_string(),
            device_long_name: "Rust GDL90 Demo".to_string(),
            capabilities: ForeFlightCapabilities {
                geometric_altitude_datum: GeometricAltitudeDatum::MeanSeaLevel,
                internet_policy: InternetPolicy::Disallowed,
                reserved_bits: 0,
            },
        }),
        Message::ForeFlightAhrs(ForeFlightAhrsMessage {
            roll_tenths_degrees: Some(((tick as i16 % 20) - 10) * 5),
            pitch_tenths_degrees: Some(0),
            heading: Some(Heading {
                heading_type: HeadingType::Magnetic,
                tenths_degrees: ((tick * 15) % 3600) as i16,
            }),
            indicated_airspeed_knots: Some(105),
            true_airspeed_knots: Some(112),
        }),
    ]
}

fn print_usage() {
    println!("gdl90 CLI");
    println!();
    println!("commands:");
    println!("  decode-frame <hex-frame>");
    println!("  decode-stream <hex-stream>");
    println!("  decode-file <session-file>");
    println!("  report-file <session-file>");
    println!("  report-file-json <session-file> [output-file]");
    println!("  support-status [--missing]");
    println!("  interface-profiles");
    println!("  analyze-file <session-file>");
    println!("  validate-file <session-file>");
    println!("  listen [bind-addr]");
    println!("  discover [bind-addr] [timeout-seconds]");
    println!("  capture [bind-addr] <output-file> [count]");
    println!("  send-demo <target-host:port> [count] [interval-ms]");
    println!("  replay-file <session-file> <target-host:port> [default-interval-ms]");
}
