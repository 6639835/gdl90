from __future__ import annotations

from common import MARKER, read, replace_once, replace_regex, write

if MARKER.exists():
    raise SystemExit(0)

write(
    "src/session.rs",
    r'''
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;

use crate::error::{Gdl90Error, Result};
use crate::message::{FrameMessageDecoder, Message};
use crate::transport::DEFAULT_MAX_DATAGRAM_SIZE;

pub const DEFAULT_MAX_SESSION_FILE_BYTES: u64 = 64 * 1024 * 1024;
pub const DEFAULT_MAX_SESSION_LINE_BYTES: usize = 8 * 1024;
pub const DEFAULT_MAX_SESSION_DATAGRAMS: usize = 1_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionReadLimits {
    pub max_file_bytes: u64,
    pub max_line_bytes: usize,
    pub max_datagrams: usize,
    pub max_datagram_bytes: usize,
}

impl Default for SessionReadLimits {
    fn default() -> Self {
        Self {
            max_file_bytes: DEFAULT_MAX_SESSION_FILE_BYTES,
            max_line_bytes: DEFAULT_MAX_SESSION_LINE_BYTES,
            max_datagrams: DEFAULT_MAX_SESSION_DATAGRAMS,
            max_datagram_bytes: DEFAULT_MAX_DATAGRAM_SIZE,
        }
    }
}

impl SessionReadLimits {
    fn validate(self) -> Result<Self> {
        if self.max_file_bytes == 0
            || self.max_line_bytes == 0
            || self.max_datagrams == 0
            || self.max_datagram_bytes == 0
        {
            return Err(Gdl90Error::InvalidField {
                field: "session read limits",
                details: "all limits must be greater than zero".to_string(),
            });
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedDatagram {
    pub delay_ms: Option<u64>,
    pub bytes: Vec<u8>,
}

impl RecordedDatagram {
    pub fn decode_messages(&self) -> Vec<Result<Message>> {
        let mut decoder = FrameMessageDecoder::new();
        let mut messages = decoder.push(&self.bytes);
        if let Some(result) = decoder.finish() {
            messages.push(result);
        }
        messages
    }

    pub fn to_line(&self) -> String {
        let hex = encode_hex(&self.bytes);
        match self.delay_ms {
            Some(delay_ms) => format!("@{delay_ms} {hex}"),
            None => hex,
        }
    }
}

pub fn read_datagram_file(path: impl AsRef<Path>) -> Result<Vec<RecordedDatagram>> {
    read_datagram_file_with_limits(path, SessionReadLimits::default())
}

pub fn read_datagram_file_with_limits(
    path: impl AsRef<Path>,
    limits: SessionReadLimits,
) -> Result<Vec<RecordedDatagram>> {
    let limits = limits.validate()?;
    let file = File::open(path.as_ref()).map_err(|error| Gdl90Error::Io {
        context: "open datagram file",
        details: error.to_string(),
    })?;
    let metadata = file.metadata().map_err(|error| Gdl90Error::Io {
        context: "read datagram file metadata",
        details: error.to_string(),
    })?;
    if metadata.len() > limits.max_file_bytes {
        return Err(Gdl90Error::ResourceLimit {
            resource: "session file bytes",
            limit: usize::try_from(limits.max_file_bytes).unwrap_or(usize::MAX),
        });
    }

    let mut reader = BufReader::new(file);
    let mut datagrams = Vec::new();
    let mut line = String::new();
    let mut line_number = 0usize;
    loop {
        line.clear();
        let bytes_read = reader
            .read_line(&mut line)
            .map_err(|error| Gdl90Error::Io {
                context: "read datagram file",
                details: error.to_string(),
            })?;
        if bytes_read == 0 {
            break;
        }
        line_number += 1;
        if bytes_read > limits.max_line_bytes {
            return Err(Gdl90Error::ResourceLimit {
                resource: "session line bytes",
                limit: limits.max_line_bytes,
            });
        }
        if let Some(datagram) = parse_datagram_line_with_limit(&line, limits.max_datagram_bytes)
            .map_err(|error| Gdl90Error::InvalidField {
                field: "datagram file line",
                details: format!("line {line_number}: {error}"),
            })?
        {
            if datagrams.len() >= limits.max_datagrams {
                return Err(Gdl90Error::ResourceLimit {
                    resource: "session datagram count",
                    limit: limits.max_datagrams,
                });
            }
            datagrams.push(datagram);
        }
    }

    Ok(datagrams)
}

pub fn write_datagram_file(path: impl AsRef<Path>, datagrams: &[RecordedDatagram]) -> Result<()> {
    let file = File::create(path.as_ref()).map_err(|error| Gdl90Error::Io {
        context: "create datagram file",
        details: error.to_string(),
    })?;
    let mut writer = BufWriter::new(file);
    for datagram in datagrams {
        writeln!(writer, "{}", datagram.to_line()).map_err(|error| Gdl90Error::Io {
            context: "write datagram file",
            details: error.to_string(),
        })?;
    }
    writer.flush().map_err(|error| Gdl90Error::Io {
        context: "flush datagram file",
        details: error.to_string(),
    })
}

pub fn append_datagram(path: impl AsRef<Path>, datagram: &RecordedDatagram) -> Result<()> {
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path.as_ref())
        .map_err(|error| Gdl90Error::Io {
            context: "open datagram file for append",
            details: error.to_string(),
        })?;
    let mut writer = BufWriter::new(file);
    writeln!(writer, "{}", datagram.to_line()).map_err(|error| Gdl90Error::Io {
        context: "append datagram file",
        details: error.to_string(),
    })?;
    writer.flush().map_err(|error| Gdl90Error::Io {
        context: "flush datagram append",
        details: error.to_string(),
    })
}

pub fn parse_datagram_line(line: &str) -> Result<Option<RecordedDatagram>> {
    parse_datagram_line_with_limit(line, DEFAULT_MAX_DATAGRAM_SIZE)
}

pub fn parse_datagram_line_with_limit(
    line: &str,
    max_datagram_bytes: usize,
) -> Result<Option<RecordedDatagram>> {
    if max_datagram_bytes == 0 {
        return Err(Gdl90Error::InvalidField {
            field: "maximum datagram bytes",
            details: "must be greater than zero".to_string(),
        });
    }
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return Ok(None);
    }

    let (delay_ms, hex) = if let Some(rest) = trimmed.strip_prefix('@') {
        let mut parts = rest.splitn(2, char::is_whitespace);
        let delay_text = parts
            .next()
            .filter(|value| !value.is_empty())
            .ok_or(Gdl90Error::InvalidField {
                field: "datagram delay",
                details: "missing delay value".to_string(),
            })?;
        let hex = parts
            .next()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or(Gdl90Error::InvalidField {
                field: "datagram line",
                details: "missing hex payload after delay".to_string(),
            })?;
        let delay_ms = delay_text
            .parse::<u64>()
            .map_err(|error| Gdl90Error::InvalidField {
                field: "datagram delay",
                details: format!("invalid delay: {error}"),
            })?;
        (Some(delay_ms), hex)
    } else {
        (None, trimmed)
    };

    let bytes = decode_hex_with_limit(hex, max_datagram_bytes)?;
    Ok(Some(RecordedDatagram { delay_ms, bytes }))
}

pub fn decode_hex(input: &str) -> Result<Vec<u8>> {
    decode_hex_with_limit(input, usize::MAX / 2)
}

pub fn decode_hex_with_limit(input: &str, max_bytes: usize) -> Result<Vec<u8>> {
    let significant_digits = input
        .chars()
        .filter(|ch| !ch.is_ascii_whitespace() && *ch != ':' && *ch != '-')
        .count();
    if significant_digits > max_bytes.saturating_mul(2) {
        return Err(Gdl90Error::ResourceLimit {
            resource: "decoded hex bytes",
            limit: max_bytes,
        });
    }
    let filtered = input
        .chars()
        .filter(|ch| !ch.is_ascii_whitespace() && *ch != ':' && *ch != '-')
        .collect::<String>();
    if filtered.is_empty() {
        return Err(Gdl90Error::InvalidField {
            field: "hex input",
            details: "input is empty".to_string(),
        });
    }
    if filtered.len() % 2 != 0 {
        return Err(Gdl90Error::InvalidField {
            field: "hex input",
            details: "must contain an even number of digits".to_string(),
        });
    }

    let mut out = Vec::with_capacity(filtered.len() / 2);
    let bytes = filtered.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        let pair = std::str::from_utf8(&bytes[index..index + 2]).map_err(|_| {
            Gdl90Error::InvalidField {
                field: "hex input",
                details: format!("byte pair at offset {index} is not valid UTF-8"),
            }
        })?;
        let value = u8::from_str_radix(pair, 16).map_err(|error| Gdl90Error::InvalidField {
            field: "hex input",
            details: format!("invalid hex byte at offset {index}: {error}"),
        })?;
        out.push(value);
        index += 2;
    }
    Ok(out)
}

pub fn encode_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(&mut out, "{byte:02X}");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_and_delayed_lines() {
        let plain = parse_datagram_line("7E 00 01 7E").unwrap().unwrap();
        assert_eq!(plain.delay_ms, None);
        assert_eq!(plain.bytes, vec![0x7E, 0x00, 0x01, 0x7E]);

        let delayed = parse_datagram_line("@250 7E00017E").unwrap().unwrap();
        assert_eq!(delayed.delay_ms, Some(250));
        assert_eq!(delayed.bytes, vec![0x7E, 0x00, 0x01, 0x7E]);
    }

    #[test]
    fn ignores_comments_and_blank_lines() {
        assert_eq!(parse_datagram_line("# comment").unwrap(), None);
        assert_eq!(parse_datagram_line("   ").unwrap(), None);
    }

    #[test]
    fn hex_encode_round_trips() {
        let bytes = vec![0x7E, 0x7D, 0x20, 0x00];
        let encoded = encode_hex(&bytes);
        assert_eq!(decode_hex(&encoded).unwrap(), bytes);
    }

    #[test]
    fn bounded_hex_decode_rejects_oversized_datagrams() {
        assert!(matches!(
            decode_hex_with_limit("000102", 2),
            Err(Gdl90Error::ResourceLimit {
                resource: "decoded hex bytes",
                limit: 2
            })
        ));
    }
}
''',
)

replace_once(
    "Cargo.toml",
    '''[package]\nname = "gdl90"\nversion = "0.1.0"\nedition = "2024"''',
    '''[package]\nname = "gdl90"\nversion = "0.1.0"\nedition = "2024"\nrust-version = "1.85"\ndescription = "Bounded GDL90 and ForeFlight protocol codecs and transport helpers"\nlicense = "MIT"\nrepository = "https://github.com/6639835/gdl90"\nreadme = "README.md"\nkeywords = ["gdl90", "ads-b", "aviation", "foreflight", "fis-b"]\ncategories = ["aerospace", "encoding", "network-programming"]''',
)

replace_once(
    "src/lib.rs",
    "//! GDL90 binary protocol and ForeFlight extension support.",
    "#![forbid(unsafe_code)]\n\n//! GDL90 binary protocol and ForeFlight extension support.\n//!\n//! This crate is not FAA-certified and must not be treated as a safety-of-flight component.",
)

support_replacements = [
    (
        '''            "2",\n            "RS-422 Bus Message Structure",\n            SupportState::Complete,\n            "The documented framing, transport characteristics, and bandwidth behavior are implemented.",''',
        '''            "2",\n            "RS-422 Bus Message Structure",\n            SupportState::Partial,\n            "Framing and bounded scheduling helpers are implemented; hardware RS-422 I/O and installation certification are not part of this crate.",''',
    ),
    (
        '''            "2.1",\n            "Physical Interface",\n            SupportState::Complete,\n            "RS-422 serial profile and connector mapping are represented in support.rs.",''',
        '''            "2.1",\n            "Physical Interface",\n            SupportState::Partial,\n            "Electrical profile and connector metadata are represented; no hardware driver, wiring validation, or certified installation support is provided.",''',
    ),
    (
        '''            "2.3",\n            "Bandwidth Management",\n            SupportState::Complete,\n            "Byte-budget scheduling and documented output order are implemented.",''',
        '''            "2.3",\n            "Bandwidth Management",\n            SupportState::Partial,\n            "A validated one-second selection algorithm is implemented; the application must supply a monotonic transmission loop and backpressure policy.",''',
    ),
    (
        '''            "3",\n            "Message Definitions",\n            SupportState::Complete,\n            "All documented outer message formats, including pass-through ADS-B inner field decoding, are implemented.",''',
        '''            "3",\n            "Message Definitions",\n            SupportState::Partial,\n            "Garmin outer message formats are implemented and pass-through payload types are validated; full inner UAT conformance depends on RTCA/DO-282 material outside the public ICD.",''',
    ),
    (
        '''            "3.6",\n            "Pass-Through Reports",\n            SupportState::Complete,\n            "Basic and Long payloads validate the matching UAT payload type and decode into typed UAT header, state-vector, mode-status, and auxiliary-state-vector fields while preserving the original raw bytes.",''',
        '''            "3.6",\n            "Pass-Through Reports",\n            SupportState::BlockedByExternalSpec,\n            "Outer lengths and matching Basic/Long payload types are validated without panic; complete inner-field conformance requires licensed RTCA/DO-282 requirements and vectors.",''',
    ),
    (
        '''            "4",\n            "Uplink Payload Format",\n            SupportState::Complete,\n            "The Rev A uplink container, information frames, APDU headers, product-file segmentation, and the product definitions supplied by Garmin are implemented; external future-product schemas are preserved raw by design.",''',
        '''            "4",\n            "Uplink Payload Format",\n            SupportState::Partial,\n            "The fixed container, zero-fill, I-Frames, minimal APDUs, and bounded reassembly are implemented; optional descriptors and externally defined products are preserved losslessly when not semantically decoded.",''',
    ),
    (
        '''            "4.3",\n            "FIS-B Product Encoding (APDUs)",\n            SupportState::Complete,\n            "Variable-length APDU headers, civil-time variants, reserved A/G/P bits, segmentation metadata, total-length constraints, and stateful product-file reassembly are implemented.",''',
        '''            "4.3",\n            "FIS-B Product Encoding (APDUs)",\n            SupportState::Partial,\n            "Minimal APDU headers and bounded source-scoped reassembly are implemented. Optional Product Descriptor forms are retained as opaque bytes pending the external normative schema.",''',
    ),
    (
        '''            "4.3.1",\n            "APDU Header",\n            SupportState::Complete,\n            "APDU headers decode and encode every operational UAT time/segmentation variant, validate civil/calendar ranges, and require the historical A/G/P positions to be zero as reserved bits.",''',
        '''            "4.3.1",\n            "APDU Header",\n            SupportState::BlockedByExternalSpec,\n            "The public minimal header is decoded semantically. Nonzero optional descriptor flags are no longer rejected or discarded and are preserved losslessly until the external descriptor schema is available.",''',
    ),
    (
        '''            "4.4",\n            "FIS-B Products",\n            SupportState::Complete,\n            "The Generic Text and NEXRAD schemas defined by Rev A are decoded and encoded; other registry product IDs are named where known and preserved losslessly because their schemas are outside the Garmin ICD.",''',
        '''            "4.4",\n            "FIS-B Products",\n            SupportState::BlockedByExternalSpec,\n            "Generic Text and the public NEXRAD examples are implemented; complete product-registry conformance requires FAA/RTCA schemas not contained in Garmin Rev A.",''',
    ),
    (
        '''            "5",\n            "FIS-B Product APDU Definition",\n            SupportState::Complete,\n            "Both product definitions supplied by the Garmin ICD—Type 4 NEXRAD and Generic Text Type 2—are implemented and covered by published sample vectors.",''',
        '''            "5",\n            "FIS-B Product APDU Definition",\n            SupportState::BlockedByExternalSpec,\n            "The Garmin examples and public fields are implemented, but the ICD delegates normative product details to RTCA/DO-267A and the FAA registry.",''',
    ),
    (
        '''            "6",\n            "Control Panel Interface",\n            SupportState::Complete,\n            "The RS-232 control-panel serial profile and all documented ASCII control messages are implemented.",''',
        '''            "6",\n            "Control Panel Interface",\n            SupportState::Partial,\n            "ASCII message codecs and cadence metadata are implemented; physical serial transport and certified equipment integration are not provided.",''',
    ),
    (
        '''            "6.1",\n            "Physical Interface",\n            SupportState::Complete,\n            "RS-232 serial profiles at 1200 and 9600 baud plus the documented DB15/P1 pin mapping are represented in support.rs.",''',
        '''            "6.1",\n            "Physical Interface",\n            SupportState::Partial,\n            "Baud/framing and pin metadata are represented, but this crate does not open or validate a physical RS-232 device.",''',
    ),
    (
        '''            "ForeFlight Connectivity",\n            "ForeFlight Connectivity",\n            SupportState::Complete,\n            "Connectivity helper logic for Heartbeat/Ownship presence and the documented packet-size guard are implemented.",''',
        '''            "ForeFlight Connectivity",\n            "ForeFlight Connectivity",\n            SupportState::Partial,\n            "Connectivity detection, IP-aware packet budgets, and a deterministic cadence scheduler are implemented; an application must drive the live loop and verify device interoperability.",''',
    ),
    (
        '''            "ForeFlight Broadcast",\n            "ForeFlight Broadcast",\n            SupportState::Complete,\n            "UDP discovery JSON parsing, configurable target derivation, and the documented 5-second cadence constant are implemented.",''',
        '''            "ForeFlight Broadcast",\n            "ForeFlight Broadcast",\n            SupportState::Partial,\n            "Discovery parsing, target derivation, and a five-second scheduler are implemented; continuous broadcast lifecycle and network-change handling belong to the embedding application.",''',
    ),
]
for old, new in support_replacements:
    replace_once("src/support.rs", old, new)

replace_once(
    "src/support.rs",
    '''    #[test]\n    fn garmin_rev_a_has_no_incomplete_support_entries() {\n        assert!(missing_sections().is_empty());\n    }''',
    '''    #[test]\n    fn support_matrix_exposes_known_external_and_transport_boundaries() {\n        let missing = missing_sections();\n        for section in ["2.1", "3.6", "4.3.1", "4.4", "6.1"] {\n            assert!(missing.iter().any(|entry| entry.section == section));\n        }\n    }''',
)

replace_once(
    "src/bin/gdl90.rs",
    "use gdl90::analysis::{analyze_datagrams, validate_datagrams};",
    "use gdl90::analysis::{analyze_datagrams, validate_datagrams_syntax};",
)
replace_once(
    "src/bin/gdl90.rs",
    "            let validation = validate_datagrams(&datagrams);",
    "            let validation = validate_datagrams_syntax(&datagrams);",
)
replace_once(
    "src/bin/gdl90.rs",
    '                return Err("session validation failed".into());',
    '                return Err("session syntax validation failed".into());',
)
replace_once(
    "src/bin/gdl90.rs",
    ".unwrap_or(5);\n            let interval_ms",
    ".unwrap_or(25);\n            let interval_ms",
)
replace_once(
    "src/bin/gdl90.rs",
    ".unwrap_or(1_000);\n            let sender = ForeFlightUdpSender",
    ".unwrap_or(200);\n            let sender = ForeFlightUdpSender",
)

replace_regex(
    "README.md",
    r'''## Protocol scope and completion.*?## License''',
    r'''## Protocol scope and production status

This crate implements a substantial and tested subset of Garmin GDL90 Rev A and the ForeFlight extension, but it does **not** claim certification, safety-of-flight suitability, or complete conformance to external RTCA and FAA product specifications.

The production hardening in this repository includes:

- bounded streaming frames, UDP datagrams, session files, and APDU reassembly state
- independent decoding for each UDP datagram and source-safe APDU reassembly APIs
- strict Basic/Long pass-through payload-type validation with no reporting panic path
- strict Garmin VFOM behavior plus an explicit ForeFlight compatibility mode
- Garmin-required zero-fill validation for unused UAT Application Data
- lossless preservation of optional Product Descriptor APDUs that cannot be decoded from the public ICD alone
- IP-version-aware ForeFlight UDP payload limits and a deterministic 5 Hz AHRS cadence scheduler
- fallible bandwidth configuration and checked byte accounting
- CI, adversarial parser tests, and scheduled fuzzing targets

Known boundaries are intentionally reported by `support-status --missing`:

- physical RS-422/RS-232 drivers and certified installation behavior are not implemented
- complete Basic/Long UAT semantics require RTCA/DO-282
- complete APDU/Product Descriptor and FIS-B product semantics require RTCA/DO-267A and the FAA FIS-B Product Registry
- real ForeFlight interoperability still requires testing against supported iOS/iPadOS releases and representative networks

See `docs/CONFORMANCE.md`, `docs/PRODUCTION_READINESS.md`, and `SECURITY.md` before embedding this crate in operational equipment.

## License''',
)

write(
    "docs/CONFORMANCE.md",
    r'''
# Conformance scope

## Normative sources

The implementation is reviewed against:

- Garmin, **GDL 90 Data Interface Specification, Public ICD, Rev A**: <https://www.faa.gov/sites/faa.gov/files/air_traffic/technology/adsb/archival/GDL90_Public_ICD_RevA.PDF>
- ForeFlight, **GDL 90 Extended Specification**: <https://www.foreflight.com/connect/spec/>

Garmin Rev A delegates portions of Basic/Long UAT payloads and FIS-B product encoding to RTCA/DO-282, RTCA/DO-267A, and the FAA FIS-B Product Registry. Those externally controlled requirements are not reconstructed or guessed here.

## Implemented guarantees

- Fixed-length Garmin outer messages validate length, reserved fields, ranges, and documented sentinels.
- Basic and Long pass-through messages validate their required inner UAT payload type before entering the typed `Message` model.
- Streaming frame and UDP input paths have explicit memory limits and recover after malformed input.
- Each UDP datagram is decoded independently; bytes from separate datagrams or sources are never combined.
- Unused UAT Application Data must be zero-filled.
- APDU reassembly is bounded by file count, byte count, segment count, age, and caller-supplied source identity.
- Optional Product Descriptor forms are preserved losslessly instead of being rejected or guessed.
- ForeFlight payload budgets account for minimum IPv4/IPv6 and UDP headers so the complete packet remains below 1500 bytes.

## Explicit compatibility decisions

Garmin Rev A assigns `0x7FFE` to geometric VFOM greater than 32766 m. ForeFlight's published page currently shows `0x7EEE`. Strict decoding follows Garmin. `decode_foreflight_compatible` and `encode_for_foreflight` make the conflicting legacy behavior opt-in and testable.

## Evidence policy

A support-matrix entry is `Complete` only when the public normative source contains enough detail for implementation and the repository includes positive and negative tests. Externally delegated schemas are marked `BlockedByExternalSpec`; physical or lifecycle behavior outside the codec is marked `Partial`.
''',
)

write(
    "docs/PRODUCTION_READINESS.md",
    r'''
# Production readiness

## What this repository now protects

- Arbitrary network input cannot grow the streaming frame buffer without bound.
- UDP packet boundaries and sender identities are not mixed.
- Oversized UDP datagrams and session inputs fail closed with structured errors.
- Pass-through report decoding and reporting do not use `expect` on attacker-controlled bytes.
- Reassembly state has configurable count, byte, segment, age, and source limits.
- Invalid bandwidth configuration returns an error instead of dividing by zero or overflowing.
- CI requires formatting, all tests, adversarial tests, and warnings-as-errors Clippy.

## Embedding application responsibilities

The embedding application must still provide:

- monotonic scheduling and backpressure integration around the cadence and bandwidth helpers
- network-interface change handling, discovery lifecycle, logging, metrics, and shutdown behavior
- serial drivers and installation-specific electrical validation when RS-422/RS-232 is required
- reassembly source identifiers derived from a stable station or transport identity
- interoperability testing against the exact ForeFlight release and network environment being supported
- licensed external specifications and their conformance vectors when claiming full UAT/FIS-B compliance

## Aviation limitation

This project is not FAA-approved, is not a TSO authorization, and has not undergone a DO-178C software life-cycle. It must not be represented as certified safety-of-flight software. Operational use requires an independent system safety assessment, requirements traceability, hardware-in-the-loop testing, fault injection, soak testing, and the approvals applicable to the equipment.
''',
)

write(
    "SECURITY.md",
    r'''
# Security policy

## Reporting

Report parser crashes, unbounded memory growth, packet-boundary confusion, integer overflow, or protocol validation bypasses through a private security advisory in this repository. Include the smallest reproducing byte sequence and the affected API.

## Supported input assumptions

All public byte-decoding APIs must tolerate malformed input without panic. Live network helpers additionally enforce bounded frame and datagram sizes. Callers should keep the defaults unless a larger limit is justified and separately tested.

## Aviation disclaimer

Security fixes and passing tests do not make this project certified avionics. Do not use it as the sole source of traffic, terrain, weather, navigation, or collision-avoidance information.
''',
)

write(
    "tests/adversarial.rs",
    r'''
use gdl90::frame::{FrameDecoder, decode_frame};
use gdl90::message::{FrameMessageDecoder, Message};
use gdl90::uplink::{ApduPayload, UatUplinkPayload};

fn next(seed: &mut u64) -> u8 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 7;
    *seed ^= *seed << 17;
    *seed as u8
}

#[test]
fn deterministic_malformed_corpus_never_panics_or_grows_without_bound() {
    let mut seed = 0xC0FF_EE12_3456_789Au64;
    for length in 0..=1_024usize {
        let mut bytes = vec![0u8; length];
        for byte in &mut bytes {
            *byte = next(&mut seed);
        }

        let _ = decode_frame(&bytes);
        if let Ok(message) = Message::decode(&bytes) {
            let _ = message.summary();
            let _ = message.encode();
        }
        let _ = ApduPayload::decode(&bytes);
        if bytes.len() == 432 {
            let _ = UatUplinkPayload::decode(&bytes);
        }

        let mut frame_decoder = FrameDecoder::with_max_stuffed_frame_len(512).unwrap();
        let _ = frame_decoder.push(&bytes);
        let _ = frame_decoder.finish();

        let mut message_decoder = FrameMessageDecoder::with_max_stuffed_frame_len(512).unwrap();
        let _ = message_decoder.push(&bytes);
        let _ = message_decoder.finish();
    }
}
''',
)

write(
    ".github/workflows/ci.yml",
    r'''
name: CI

on:
  pull_request:
  push:
    branches: [main]

permissions:
  contents: read

jobs:
  quality:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      - name: Formatting
        run: cargo fmt --all -- --check
      - name: Tests
        run: cargo test --all-targets
      - name: Clippy
        run: cargo clippy --all-targets -- -D warnings
''',
)

write(
    ".github/workflows/fuzz.yml",
    r'''
name: Scheduled fuzzing

on:
  workflow_dispatch:
  schedule:
    - cron: "17 3 * * 1"

permissions:
  contents: read

jobs:
  fuzz:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@nightly
      - run: cargo install cargo-fuzz --locked
      - working-directory: fuzz
        run: cargo +nightly fuzz run frame_and_message -- -max_total_time=120
      - working-directory: fuzz
        run: cargo +nightly fuzz run uplink -- -max_total_time=120
''',
)

write(
    "fuzz/Cargo.toml",
    r'''
[package]
name = "gdl90-fuzz"
version = "0.0.0"
publish = false
edition = "2024"

[package.metadata]
cargo-fuzz = true

[dependencies]
libfuzzer-sys = "0.4"
gdl90 = { path = ".." }

[[bin]]
name = "frame_and_message"
path = "fuzz_targets/frame_and_message.rs"
test = false
doc = false
bench = false

[[bin]]
name = "uplink"
path = "fuzz_targets/uplink.rs"
test = false
doc = false
bench = false

[workspace]
members = ["."]
''',
)

write(
    "fuzz/fuzz_targets/frame_and_message.rs",
    r'''
#![no_main]

use gdl90::frame::{FrameDecoder, decode_frame};
use gdl90::{FrameMessageDecoder, Message};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = decode_frame(data);
    if let Ok(message) = Message::decode(data) {
        let _ = message.summary();
        let _ = message.encode();
    }
    let mut frames = FrameDecoder::with_max_stuffed_frame_len(1_024).unwrap();
    let _ = frames.push(data);
    let _ = frames.finish();
    let mut messages = FrameMessageDecoder::with_max_stuffed_frame_len(1_024).unwrap();
    let _ = messages.push(data);
    let _ = messages.finish();
});
''',
)

write(
    "fuzz/fuzz_targets/uplink.rs",
    r'''
#![no_main]

use gdl90::uplink::{ApduPayload, UatUplinkPayload};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = ApduPayload::decode(data);
    if data.len() == 432 {
        if let Ok(payload) = UatUplinkPayload::decode(data) {
            let _ = payload.decoded_header();
            let _ = payload.information_frames();
            let _ = payload.apdu_payloads();
        }
    }
});
''',
)

MARKER.write_text("applied\n", encoding="utf-8")
