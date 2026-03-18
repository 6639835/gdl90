use std::env;
use std::thread;
use std::time::Duration;

use gdl90::foreflight::{
    ForeFlightAhrsMessage, ForeFlightCapabilities, ForeFlightIdMessage, GeometricAltitudeDatum,
    Heading, HeadingType, InternetPolicy,
};
use gdl90::message::{
    AddressType, Heartbeat, HeartbeatStatus, Message, OwnshipGeometricAltitude, TargetAlertStatus,
    TargetMisc, TargetReport, TrackType, VerticalFigureOfMerit,
};
use gdl90::transport::{
    FOREFLIGHT_DISCOVERY_PORT, FOREFLIGHT_GDL90_PORT, UdpGdl90Receiver, UdpGdl90Sender,
    discover_foreflight_once,
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
            let bytes = decode_hex(&hex)?;
            let clear = gdl90::frame::decode_frame(&bytes)?;
            let message = Message::decode(&clear)?;
            println!("{message:#?}");
        }
        Some("decode-stream") => {
            let hex = require_arg(args.next(), "hex stream")?;
            let bytes = decode_hex(&hex)?;
            let mut decoder = gdl90::FrameMessageDecoder::new();
            for result in decoder.push(&bytes) {
                println!("{:#?}", result?);
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
                    "suggested target: {}:{}",
                    source.ip(),
                    announcement.gdl90_port
                );
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
            let sender = UdpGdl90Sender::bind("0.0.0.0:0", &target)?;
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
        _ => print_usage(),
    }

    Ok(())
}

fn require_arg(value: Option<String>, name: &'static str) -> Result<String, String> {
    value.ok_or_else(|| format!("missing required argument: {name}"))
}

fn decode_hex(input: &str) -> Result<Vec<u8>, String> {
    let filtered = input
        .chars()
        .filter(|ch| !ch.is_ascii_whitespace() && *ch != ':' && *ch != '-')
        .collect::<String>();
    if filtered.len() % 2 != 0 {
        return Err("hex input must contain an even number of digits".to_string());
    }

    let mut out = Vec::with_capacity(filtered.len() / 2);
    let bytes = filtered.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        let pair =
            std::str::from_utf8(&bytes[index..index + 2]).map_err(|error| error.to_string())?;
        let value = u8::from_str_radix(pair, 16).map_err(|error| error.to_string())?;
        out.push(value);
        index += 2;
    }
    Ok(out)
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
    println!("  listen [bind-addr]");
    println!("  discover [bind-addr] [timeout-seconds]");
    println!("  send-demo <target-host:port> [count] [interval-ms]");
}
