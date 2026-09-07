use std::env;
use std::io::{self, Write};
use std::net::{SocketAddr, ToSocketAddrs};
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use gdl90::analysis::{analyze_datagrams, validate_datagrams_syntax};
use gdl90::foreflight::{
    ForeFlightAhrsMessage, ForeFlightCapabilities, ForeFlightIdMessage, GeometricAltitudeDatum,
    Heading, HeadingType, InternetPolicy,
};
use gdl90::message::{
    AddressType, Heartbeat, HeartbeatStatus, Message, OwnshipGeometricAltitude, TargetAlertStatus,
    TargetMisc, TargetReport, TrackType, VerticalFigureOfMerit,
};
use gdl90::report::{
    build_session_report, render_analysis_text, render_json_report, render_text_report,
    render_validation_text, write_json_report,
};
use gdl90::session::{
    DatagramFileWriter, RecordedDatagram, SessionWriteLimits, decode_hex, read_datagram_file,
};
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
        let broken_pipe = error
            .downcast_ref::<io::Error>()
            .is_some_and(|error| error.kind() == io::ErrorKind::BrokenPipe)
            || error
                .downcast_ref::<gdl90::Gdl90Error>()
                .is_some_and(|error| error.io_kind() == Some(io::ErrorKind::BrokenPipe));
        if broken_pipe {
            return;
        }
        let _ = writeln!(io::stderr().lock(), "error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<_> = env::args().skip(1).collect();
    validate_arguments(&arguments)?;
    let mut args = arguments.into_iter();
    let stdout = io::stdout();
    let mut out = stdout.lock();
    match args.next().as_deref() {
        Some("decode-frame") => {
            let hex = require_arg(args.next(), "hex frame")?;
            let bytes = decode_hex(&hex)?;
            let clear = gdl90::frame::decode_frame(&bytes)?;
            let message = Message::decode(&clear)?;
            writeln!(out, "{message:#?}")?;
        }
        Some("decode-stream") => {
            let hex = require_arg(args.next(), "hex stream")?;
            let bytes = decode_hex(&hex)?;
            let mut decoder = gdl90::FrameMessageDecoder::new();
            for result in decoder.push(&bytes) {
                writeln!(out, "{:#?}", result?)?;
            }
            if let Some(result) = decoder.finish() {
                writeln!(out, "{:#?}", result?)?;
            }
        }
        Some("decode-file") => {
            let path = PathBuf::from(require_arg(args.next(), "session file")?);
            let datagrams = read_datagram_file(&path)?;
            for (index, datagram) in datagrams.iter().enumerate() {
                writeln!(
                    out,
                    "datagram {} delay={:?} bytes={}",
                    index + 1,
                    datagram.delay_ms,
                    datagram.bytes.len()
                )?;
                for message in datagram.decode_messages() {
                    match message {
                        Ok(message) => writeln!(out, "{message:#?}")?,
                        Err(error) => writeln!(out, "decode error: {error}")?,
                    }
                }
            }
        }
        Some("report-file") => {
            let path = PathBuf::from(require_arg(args.next(), "session file")?);
            let datagrams = read_datagram_file(&path)?;
            let report = build_session_report(&datagrams);
            write!(out, "{}", render_text_report(&report))?;
        }
        Some("report-file-json") => {
            let path = PathBuf::from(require_arg(args.next(), "session file")?);
            let output = args.next().map(PathBuf::from);
            let datagrams = read_datagram_file(&path)?;
            let report = build_session_report(&datagrams);
            if let Some(output) = output {
                write_json_report(&output, &report, true)?;
                writeln!(out, "wrote {}", output.display())?;
            } else {
                let json = render_json_report(&report, true)?;
                writeln!(out, "{json}")?;
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
                writeln!(
                    out,
                    "{} [{}] {}",
                    entry.section,
                    render_support_state(entry.state),
                    entry.title
                )?;
                writeln!(out, "  {}", entry.notes)?;
            }
        }
        Some("interface-profiles") => {
            let rs422 = rs422_bus_profile();
            writeln!(
                out,
                "RS-422 bus: {:?} {} baud {}{}{} {:?} {:?}",
                rs422.signal_type,
                rs422.baud_rate,
                rs422.start_bits,
                rs422.data_bits,
                rs422.stop_bits,
                rs422.parity,
                rs422.flow_control
            )?;
            for connection in rs422_connections() {
                writeln!(
                    out,
                    "  {} | {} | {}",
                    connection.signal_name, connection.direction, connection.connector_pin
                )?;
            }
            writeln!(out, "Control panel profiles:")?;
            for profile in control_panel_profiles() {
                writeln!(
                    out,
                    "  {:?} {} baud {}{}{} {:?} {:?}",
                    profile.signal_type,
                    profile.baud_rate,
                    profile.start_bits,
                    profile.data_bits,
                    profile.stop_bits,
                    profile.parity,
                    profile.flow_control
                )?;
            }
            for connection in control_panel_connections() {
                writeln!(
                    out,
                    "  {} | {} | {}",
                    connection.signal_name, connection.direction, connection.connector_pin
                )?;
            }
        }
        Some("analyze-file") => {
            let path = PathBuf::from(require_arg(args.next(), "session file")?);
            let datagrams = read_datagram_file(&path)?;
            let analysis = analyze_datagrams(&datagrams);
            write!(out, "{}", render_analysis_text(&analysis))?;
        }
        Some("validate-file") => {
            let path = PathBuf::from(require_arg(args.next(), "session file")?);
            let datagrams = read_datagram_file(&path)?;
            let validation = validate_datagrams_syntax(&datagrams);
            write!(out, "{}", render_validation_text(&validation))?;
            if !validation.is_valid() {
                return Err("session syntax validation failed".into());
            }
        }
        Some("listen") => {
            let bind = args
                .next()
                .unwrap_or_else(|| format!("0.0.0.0:{FOREFLIGHT_GDL90_PORT}"));
            let mut receiver = UdpGdl90Receiver::bind(&bind)?;
            let mut discarded = 0u64;
            writeln!(out, "listening on {}", receiver.local_addr()?)?;
            loop {
                let datagram = receive_next(&mut receiver, &mut discarded)?;
                writeln!(
                    out,
                    "from {} ({} bytes)",
                    datagram.source,
                    datagram.bytes.len()
                )?;
                for message in datagram.messages {
                    match message {
                        Ok(message) => writeln!(out, "{message:#?}")?,
                        Err(error) => writeln!(out, "decode error: {error}")?,
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
            writeln!(out, "source: {source}")?;
            writeln!(out, "announcement: {announcement:#?}")?;
            if announcement.is_foreflight() {
                writeln!(
                    out,
                    "suggested target: {}",
                    announcement.target_for_source(source)
                )?;
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
            let mut discarded = 0u64;
            writeln!(
                out,
                "capturing on {} to {}",
                receiver.local_addr()?,
                output.display()
            )?;
            let mut writer = DatagramFileWriter::open(&output, SessionWriteLimits::default())?;
            let mut seen = 0usize;
            let mut previous = None;
            loop {
                let datagram = receive_next(&mut receiver, &mut discarded)?;
                let now = Instant::now();
                let delay = previous.map_or(0, |last| {
                    u64::try_from(now.duration_since(last).as_millis()).unwrap_or(u64::MAX)
                });
                writer.append(&RecordedDatagram {
                    delay_ms: Some(delay),
                    bytes: datagram.bytes,
                })?;
                previous = Some(now);
                seen += 1;
                writeln!(out, "captured datagram {seen} from {}", datagram.source)?;
                if count != 0 && seen >= count {
                    break;
                }
            }
            writer.sync_all()?;
        }
        Some("send-demo") => {
            let target = require_arg(args.next(), "target host:port")?;
            let count = args
                .next()
                .map(|value| value.parse::<u32>())
                .transpose()?
                .unwrap_or(25);
            let interval_ms = args
                .next()
                .map(|value| value.parse::<u64>())
                .transpose()?
                .unwrap_or(200);
            validate_delay(interval_ms)?;
            if interval_ms == 0 {
                return Err("demo interval must be greater than zero".into());
            }
            let (bind, address) = sender_endpoint(&target)?;
            let sender = ForeFlightUdpSender::bind(bind, address)?;
            writeln!(
                out,
                "sending demo traffic from {} to {}",
                sender.local_addr()?,
                target
            )?;
            for tick in 0..count {
                let seconds = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() % 86_400;
                let messages = demo_messages(tick, seconds as u32);
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
            validate_delay(default_interval_ms)?;
            for datagram in &datagrams {
                validate_delay(datagram.delay_ms.unwrap_or(0))?;
            }
            let (bind, address) = sender_endpoint(&target)?;
            let sender = UdpGdl90Sender::bind(bind, address)?;
            writeln!(
                out,
                "replaying {} datagrams from {} to {}",
                datagrams.len(),
                path.display(),
                target
            )?;
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
        _ => print_usage(&mut out)?,
    }

    Ok(())
}

fn require_arg(value: Option<String>, name: &'static str) -> Result<String, String> {
    value.ok_or_else(|| format!("missing required argument: {name}"))
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

fn demo_messages(tick: u32, timestamp: u32) -> Vec<Message> {
    // Synthetic bounded motion; long demos cannot drift outside Earth coordinates.
    let phase = f64::from(tick % 3600) * std::f64::consts::TAU / 3600.0;
    let lat = 37.6188056 + phase.sin() * 0.01;
    let lon = -122.3754167 + phase.cos() * 0.01;

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
            roll_tenths_degrees: Some(((tick % 20) as i16 - 10) * 5),
            pitch_tenths_degrees: Some(0),
            heading: Some(Heading {
                heading_type: HeadingType::Magnetic,
                tenths_degrees: ((u64::from(tick) * 15) % 3600) as i16,
            }),
            indicated_airspeed_knots: Some(105),
            true_airspeed_knots: Some(112),
        }),
    ]
}

fn print_usage(out: &mut impl Write) -> io::Result<()> {
    writeln!(out, "gdl90 CLI")?;
    writeln!(out)?;
    writeln!(out, "commands:")?;
    writeln!(out, "  decode-frame <hex-frame>")?;
    writeln!(out, "  decode-stream <hex-stream>")?;
    writeln!(out, "  decode-file <session-file>")?;
    writeln!(out, "  report-file <session-file>")?;
    writeln!(out, "  report-file-json <session-file> [output-file]")?;
    writeln!(out, "  support-status [--missing]")?;
    writeln!(out, "  interface-profiles")?;
    writeln!(out, "  analyze-file <session-file>")?;
    writeln!(out, "  validate-file <session-file>")?;
    writeln!(out, "  listen [bind-addr]")?;
    writeln!(out, "  discover [bind-addr] [timeout-seconds]")?;
    writeln!(out, "  capture <bind-addr> <output-file> [count]")?;
    writeln!(out, "  send-demo <target-host:port> [count] [interval-ms]")?;
    writeln!(
        out,
        "  replay-file <session-file> <target-host:port> [default-interval-ms]"
    )?;
    Ok(())
}

fn validate_arguments(args: &[String]) -> Result<(), String> {
    let command = args.first().map(String::as_str);
    let (min, max) = match command {
        None | Some("help" | "--help" | "-h") => (0, 0),
        Some(
            "decode-frame" | "decode-stream" | "decode-file" | "report-file" | "analyze-file"
            | "validate-file",
        ) => (1, 1),
        Some("report-file-json") => (1, 2),
        Some("support-status") => (0, 1),
        Some("interface-profiles") => (0, 0),
        Some("listen") => (0, 1),
        Some("discover") => (0, 2),
        Some("capture") => (2, 3),
        Some("send-demo") => (1, 3),
        Some("replay-file") => (2, 3),
        Some(other) => return Err(format!("unknown command {other:?}; use --help")),
    };
    let count = args.len().saturating_sub(1);
    if count < min || count > max {
        return Err(format!(
            "command requires {min}..={max} arguments; use --help"
        ));
    }
    if command == Some("support-status") && args.get(1).is_some_and(|value| value != "--missing") {
        return Err("support-status only accepts --missing".into());
    }
    Ok(())
}

fn validate_delay(milliseconds: u64) -> Result<(), String> {
    if milliseconds > 86_400_000 {
        return Err("delay exceeds the CLI's 24-hour per-datagram safety limit".into());
    }
    Ok(())
}

fn sender_endpoint(target: &str) -> Result<(&'static str, SocketAddr), Box<dyn std::error::Error>> {
    let address = target
        .to_socket_addrs()?
        .next()
        .ok_or("target resolved to no addresses")?;
    Ok((
        if address.is_ipv4() {
            "0.0.0.0:0"
        } else {
            "[::]:0"
        },
        address,
    ))
}

fn receive_next(
    receiver: &mut UdpGdl90Receiver,
    discarded: &mut u64,
) -> gdl90::Result<gdl90::transport::UdpDatagram> {
    loop {
        let result = receiver.receive();
        match result {
            Ok(datagram) if !datagram.bytes.is_empty() => return Ok(datagram),
            Err(error) if error.io_kind() == Some(io::ErrorKind::Interrupted) => continue,
            Err(error) if !matches!(error, gdl90::Gdl90Error::DatagramTooLarge { .. }) => {
                return Err(error);
            }
            _ => {
                *discarded = discarded.saturating_add(1);
                // Logarithmic diagnostics prevent an input flood from becoming a log flood.
                if discarded.is_power_of_two() {
                    let _ = writeln!(
                        io::stderr().lock(),
                        "discarded {discarded} empty or oversized UDP datagrams"
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn demo_encodes_at_counter_boundaries() {
        for tick in [0, 32_767, 32_768, u32::MAX] {
            for message in demo_messages(tick, 86_399) {
                message.encode().unwrap();
            }
        }
    }
}
