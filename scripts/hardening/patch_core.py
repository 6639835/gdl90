from __future__ import annotations

from common import MARKER, read, replace_once, write

if MARKER.exists():
    raise SystemExit(0)

write(
    "src/error.rs",
    r'''
use core::fmt;

pub type Result<T> = std::result::Result<T, Gdl90Error>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Gdl90Error {
    InvalidLength {
        context: &'static str,
        expected: &'static str,
        actual: usize,
    },
    InvalidField {
        field: &'static str,
        details: String,
    },
    InvalidMessageId(u8),
    MissingFrameFlag,
    FrameTooShort,
    FrameTooLong {
        limit: usize,
    },
    DanglingEscape,
    InvalidEscapeByte(u8),
    CrcMismatch {
        expected: u16,
        actual: u16,
    },
    DatagramTooLarge {
        limit: usize,
        actual: usize,
    },
    ResourceLimit {
        resource: &'static str,
        limit: usize,
    },
    Utf8 {
        field: &'static str,
    },
    UnsupportedCharacter {
        context: &'static str,
        ch: char,
    },
    ControlChecksumMismatch {
        expected: u8,
        actual: u8,
    },
    ControlFormat(&'static str),
    Io {
        context: &'static str,
        details: String,
    },
}

impl fmt::Display for Gdl90Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLength {
                context,
                expected,
                actual,
            } => write!(
                f,
                "{context} length mismatch: expected {expected}, got {actual}"
            ),
            Self::InvalidField { field, details } => write!(f, "invalid {field}: {details}"),
            Self::InvalidMessageId(id) => write!(f, "unsupported message id {id:#04x}"),
            Self::MissingFrameFlag => write!(f, "frame is missing start or end flag"),
            Self::FrameTooShort => write!(f, "frame is too short"),
            Self::FrameTooLong { limit } => {
                write!(f, "frame exceeds the configured {limit}-byte stuffed-frame limit")
            }
            Self::DanglingEscape => write!(f, "frame ended with a dangling escape byte"),
            Self::InvalidEscapeByte(byte) => write!(f, "invalid escaped byte {byte:#04x}"),
            Self::CrcMismatch { expected, actual } => {
                write!(
                    f,
                    "crc mismatch: expected {expected:#06x}, got {actual:#06x}"
                )
            }
            Self::DatagramTooLarge { limit, actual } => write!(
                f,
                "UDP datagram exceeds the configured {limit}-byte limit (received at least {actual} bytes)"
            ),
            Self::ResourceLimit { resource, limit } => {
                write!(f, "{resource} exceeds the configured limit of {limit}")
            }
            Self::Utf8 { field } => write!(f, "{field} is not valid UTF-8"),
            Self::UnsupportedCharacter { context, ch } => {
                write!(f, "unsupported character {ch:?} in {context}")
            }
            Self::ControlChecksumMismatch { expected, actual } => write!(
                f,
                "control checksum mismatch: expected {expected:02X}, got {actual:02X}"
            ),
            Self::ControlFormat(details) => write!(f, "invalid control message format: {details}"),
            Self::Io { context, details } => write!(f, "{context}: {details}"),
        }
    }
}

impl std::error::Error for Gdl90Error {}
''',
)

write(
    "src/frame.rs",
    r'''
use crate::error::{Gdl90Error, Result};

pub const FLAG_BYTE: u8 = 0x7E;
pub const ESCAPE_BYTE: u8 = 0x7D;
pub const ESCAPE_MASK: u8 = 0x20;

/// Safely exceeds the worst-case stuffed size of every fixed-length message
/// implemented by this crate while preventing an unterminated frame from
/// growing memory without bound.
pub const DEFAULT_MAX_STUFFED_FRAME_LEN: usize = 1_024;

pub fn crc16_ccitt(data: &[u8]) -> u16 {
    let mut crc = 0u16;
    for byte in data {
        crc = crc16_table((crc >> 8) as u8) ^ (crc << 8) ^ (*byte as u16);
    }
    crc
}

pub fn encode_frame(clear_message: &[u8]) -> Vec<u8> {
    let crc = crc16_ccitt(clear_message);
    let mut framed = Vec::with_capacity(clear_message.len() + 6);
    framed.push(FLAG_BYTE);

    for byte in clear_message
        .iter()
        .copied()
        .chain([crc as u8, (crc >> 8) as u8])
    {
        if matches!(byte, FLAG_BYTE | ESCAPE_BYTE) {
            framed.push(ESCAPE_BYTE);
            framed.push(byte ^ ESCAPE_MASK);
        } else {
            framed.push(byte);
        }
    }

    framed.push(FLAG_BYTE);
    framed
}

pub fn decode_frame(frame: &[u8]) -> Result<Vec<u8>> {
    if frame.len() < 4 {
        return Err(Gdl90Error::FrameTooShort);
    }
    if frame.first() != Some(&FLAG_BYTE) || frame.last() != Some(&FLAG_BYTE) {
        return Err(Gdl90Error::MissingFrameFlag);
    }

    let unescaped = unescape(&frame[1..frame.len() - 1])?;
    decode_clear_message(&unescaped)
}

pub(crate) fn decode_clear_message(unescaped_payload_and_crc: &[u8]) -> Result<Vec<u8>> {
    if unescaped_payload_and_crc.len() < 3 {
        return Err(Gdl90Error::FrameTooShort);
    }

    let payload_len = unescaped_payload_and_crc.len() - 2;
    let payload = &unescaped_payload_and_crc[..payload_len];
    let actual = u16::from_le_bytes([
        unescaped_payload_and_crc[payload_len],
        unescaped_payload_and_crc[payload_len + 1],
    ]);
    let expected = crc16_ccitt(payload);
    if expected != actual {
        return Err(Gdl90Error::CrcMismatch { expected, actual });
    }

    Ok(payload.to_vec())
}

pub(crate) fn unescape(data: &[u8]) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(data.len());
    let mut escaped = false;

    for byte in data {
        if escaped {
            let restored = *byte ^ ESCAPE_MASK;
            if !matches!(restored, FLAG_BYTE | ESCAPE_BYTE) {
                return Err(Gdl90Error::InvalidEscapeByte(*byte));
            }
            out.push(restored);
            escaped = false;
        } else if *byte == ESCAPE_BYTE {
            escaped = true;
        } else {
            out.push(*byte);
        }
    }

    if escaped {
        return Err(Gdl90Error::DanglingEscape);
    }

    Ok(out)
}

#[derive(Debug, Clone)]
pub struct FrameDecoder {
    collecting: bool,
    discarding_oversized: bool,
    buffer: Vec<u8>,
    max_stuffed_frame_len: usize,
}

impl Default for FrameDecoder {
    fn default() -> Self {
        Self {
            collecting: false,
            discarding_oversized: false,
            buffer: Vec::new(),
            max_stuffed_frame_len: DEFAULT_MAX_STUFFED_FRAME_LEN,
        }
    }
}

impl FrameDecoder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_max_stuffed_frame_len(max_stuffed_frame_len: usize) -> Result<Self> {
        if max_stuffed_frame_len < 3 {
            return Err(Gdl90Error::InvalidField {
                field: "maximum stuffed frame length",
                details: "must allow at least one payload byte and the two-byte FCS".to_string(),
            });
        }
        Ok(Self {
            max_stuffed_frame_len,
            ..Self::default()
        })
    }

    pub fn max_stuffed_frame_len(&self) -> usize {
        self.max_stuffed_frame_len
    }

    pub fn push(&mut self, bytes: &[u8]) -> Vec<Result<Vec<u8>>> {
        let mut frames = Vec::new();

        for byte in bytes {
            if *byte == FLAG_BYTE {
                if self.discarding_oversized {
                    frames.push(Err(Gdl90Error::FrameTooLong {
                        limit: self.max_stuffed_frame_len,
                    }));
                    self.discarding_oversized = false;
                    self.buffer.clear();
                    self.collecting = false;
                } else if self.collecting && !self.buffer.is_empty() {
                    let stuffed = std::mem::take(&mut self.buffer);
                    match unescape(&stuffed) {
                        Ok(data) => frames.push(decode_clear_message(&data)),
                        Err(error) => frames.push(Err(error)),
                    }
                    self.collecting = false;
                } else {
                    self.buffer.clear();
                    self.collecting = true;
                }
            } else if self.collecting && !self.discarding_oversized {
                if self.buffer.len() >= self.max_stuffed_frame_len {
                    self.buffer.clear();
                    self.discarding_oversized = true;
                } else {
                    self.buffer.push(*byte);
                }
            }
        }

        frames
    }

    pub fn finish(&mut self) -> Option<Result<Vec<u8>>> {
        let result = if self.discarding_oversized {
            Some(Err(Gdl90Error::FrameTooLong {
                limit: self.max_stuffed_frame_len,
            }))
        } else if self.collecting && !self.buffer.is_empty() {
            Some(Err(Gdl90Error::FrameTooShort))
        } else {
            None
        };
        self.reset();
        result
    }

    pub fn reset(&mut self) {
        self.collecting = false;
        self.discarding_oversized = false;
        self.buffer.clear();
    }
}

fn crc16_table(index: u8) -> u16 {
    let mut crc = (index as u16) << 8;
    for _ in 0..8 {
        crc = (crc << 1) ^ if (crc & 0x8000) != 0 { 0x1021 } else { 0 };
    }
    crc
}

#[cfg(test)]
mod tests {
    use super::{FrameDecoder, decode_frame, encode_frame};
    use crate::Gdl90Error;

    #[test]
    fn byte_stuffing_examples_match_public_icd_examples() {
        let stuffed_flag = encode_frame(&[0x02, 0x7E]);
        assert_eq!(&stuffed_flag[..4], &[0x7E, 0x02, 0x7D, 0x5E]);
        assert_eq!(decode_frame(&stuffed_flag).unwrap(), vec![0x02, 0x7E]);

        let stuffed_escape = encode_frame(&[0x03, 0x7D]);
        assert_eq!(&stuffed_escape[..4], &[0x7E, 0x03, 0x7D, 0x5D]);
        assert_eq!(decode_frame(&stuffed_escape).unwrap(), vec![0x03, 0x7D]);

        let stuffed_crc = encode_frame(&[0x7E, 0x7D]);
        assert!(stuffed_crc.ends_with(&[0x7D, 0x5D, 0x7D, 0x5E, 0x7E]));
        assert_eq!(decode_frame(&stuffed_crc).unwrap(), vec![0x7E, 0x7D]);
    }

    #[test]
    fn oversized_stream_frame_is_bounded_and_decoder_recovers() {
        let mut decoder = FrameDecoder::with_max_stuffed_frame_len(8).unwrap();
        let mut bytes = vec![0x7E];
        bytes.extend_from_slice(&[0; 9]);
        bytes.push(0x7E);
        bytes.extend_from_slice(&encode_frame(&[0x02, 0x00, 0x00]));

        let decoded = decoder.push(&bytes);
        assert!(matches!(
            decoded.first(),
            Some(Err(Gdl90Error::FrameTooLong { limit: 8 }))
        ));
        assert_eq!(decoded[1].as_ref().unwrap(), &[0x02, 0x00, 0x00]);
    }

    #[test]
    fn unfinished_oversized_frame_reports_limit_on_finish() {
        let mut decoder = FrameDecoder::with_max_stuffed_frame_len(4).unwrap();
        decoder.push(&[0x7E, 1, 2, 3, 4, 5]);
        assert!(matches!(
            decoder.finish(),
            Some(Err(Gdl90Error::FrameTooLong { limit: 4 }))
        ));
    }
}
''',
)

write(
    "src/transport.rs",
    r'''
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::time::Duration;

use serde::Deserialize;

use crate::error::{Gdl90Error, Result};
use crate::foreflight;
use crate::message::{FrameMessageDecoder, Message};

pub const FOREFLIGHT_DISCOVERY_PORT: u16 = 63_093;
pub const FOREFLIGHT_GDL90_PORT: u16 = 4_000;
pub const DEFAULT_MAX_DATAGRAM_SIZE: usize = 2_048;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForeFlightDiscoveryAnnouncement {
    pub app: String,
    pub gdl90_port: u16,
}

impl ForeFlightDiscoveryAnnouncement {
    pub fn parse(json: &str) -> Result<Self> {
        let envelope: ForeFlightDiscoveryEnvelope =
            serde_json::from_str(json).map_err(|error| Gdl90Error::InvalidField {
                field: "ForeFlight discovery JSON",
                details: error.to_string(),
            })?;

        Ok(Self {
            app: envelope.app,
            gdl90_port: envelope.gdl90.port,
        })
    }

    pub fn is_foreflight(&self) -> bool {
        self.app == "ForeFlight"
    }

    pub fn target_for_source(&self, source: SocketAddr) -> SocketAddr {
        SocketAddr::new(source.ip(), self.gdl90_port)
    }
}

#[derive(Debug, Deserialize)]
struct ForeFlightDiscoveryEnvelope {
    #[serde(rename = "App")]
    app: String,
    #[serde(rename = "GDL90")]
    gdl90: ForeFlightDiscoveryGdl90,
}

#[derive(Debug, Deserialize)]
struct ForeFlightDiscoveryGdl90 {
    port: u16,
}

#[derive(Debug)]
pub struct ForeFlightUdpSender {
    inner: UdpGdl90Sender,
}

impl ForeFlightUdpSender {
    pub fn bind(bind_addr: impl ToSocketAddrs, target: impl ToSocketAddrs) -> Result<Self> {
        Ok(Self {
            inner: UdpGdl90Sender::bind(bind_addr, target)?,
        })
    }

    pub fn local_addr(&self) -> Result<SocketAddr> {
        self.inner.local_addr()
    }

    pub fn socket(&self) -> &UdpSocket {
        self.inner.socket()
    }

    pub fn target(&self) -> SocketAddr {
        self.inner.target()
    }

    /// Uses the conservative IPv6 packet budget when no destination address is available.
    pub fn encode_messages(messages: &[Message]) -> Result<Vec<u8>> {
        foreflight::encode_datagram(messages)
    }

    pub fn send_message(&self, message: &Message) -> Result<usize> {
        self.send_messages(std::slice::from_ref(message))
    }

    pub fn send_messages(&self, messages: &[Message]) -> Result<usize> {
        let datagram = foreflight::encode_datagram_for_ip(messages, self.inner.target().ip())?;
        self.inner.send_frame(&datagram)
    }
}

#[derive(Debug)]
pub struct UdpGdl90Sender {
    socket: UdpSocket,
    target: SocketAddr,
}

impl UdpGdl90Sender {
    pub fn bind(bind_addr: impl ToSocketAddrs, target: impl ToSocketAddrs) -> Result<Self> {
        let socket = UdpSocket::bind(bind_addr).map_err(|error| Gdl90Error::Io {
            context: "bind UDP sender socket",
            details: error.to_string(),
        })?;
        let target = first_socket_addr(target, "resolve UDP target address")?;
        Ok(Self { socket, target })
    }

    pub fn local_addr(&self) -> Result<SocketAddr> {
        self.socket.local_addr().map_err(|error| Gdl90Error::Io {
            context: "read UDP sender local address",
            details: error.to_string(),
        })
    }

    pub fn socket(&self) -> &UdpSocket {
        &self.socket
    }

    pub fn target(&self) -> SocketAddr {
        self.target
    }

    pub fn send_message(&self, message: &Message) -> Result<usize> {
        let frame = message.encode_frame()?;
        self.send_frame(&frame)
    }

    pub fn send_messages(&self, messages: &[Message]) -> Result<usize> {
        let mut datagram = Vec::new();
        for message in messages {
            datagram.extend_from_slice(&message.encode_frame()?);
        }
        self.send_frame(&datagram)
    }

    pub fn send_frame(&self, frame: &[u8]) -> Result<usize> {
        self.socket
            .send_to(frame, self.target)
            .map_err(|error| Gdl90Error::Io {
                context: "send UDP datagram",
                details: error.to_string(),
            })
    }
}

#[derive(Debug)]
pub struct UdpGdl90Receiver {
    socket: UdpSocket,
    max_datagram_size: usize,
}

#[derive(Debug)]
pub struct UdpDatagram {
    pub source: SocketAddr,
    pub bytes: Vec<u8>,
    pub messages: Vec<Result<Message>>,
}

/// Decodes exactly one UDP datagram. Decoder state is deliberately not shared
/// across datagrams or source addresses, so packet loss cannot splice frames.
pub fn decode_datagram(bytes: &[u8]) -> Vec<Result<Message>> {
    let mut decoder = FrameMessageDecoder::new();
    let mut messages = decoder.push(bytes);
    if let Some(result) = decoder.finish() {
        messages.push(result);
    }
    messages
}

impl UdpGdl90Receiver {
    pub fn bind(bind_addr: impl ToSocketAddrs) -> Result<Self> {
        let socket = UdpSocket::bind(bind_addr).map_err(|error| Gdl90Error::Io {
            context: "bind UDP receiver socket",
            details: error.to_string(),
        })?;
        Ok(Self {
            socket,
            max_datagram_size: DEFAULT_MAX_DATAGRAM_SIZE,
        })
    }

    pub fn local_addr(&self) -> Result<SocketAddr> {
        self.socket.local_addr().map_err(|error| Gdl90Error::Io {
            context: "read UDP receiver local address",
            details: error.to_string(),
        })
    }

    pub fn socket(&self) -> &UdpSocket {
        &self.socket
    }

    pub fn set_read_timeout(&self, timeout: Option<Duration>) -> Result<()> {
        self.socket
            .set_read_timeout(timeout)
            .map_err(|error| Gdl90Error::Io {
                context: "set UDP receiver read timeout",
                details: error.to_string(),
            })
    }

    pub fn set_max_datagram_size(&mut self, size: usize) {
        self.max_datagram_size = size.max(1);
    }

    pub fn receive(&mut self) -> Result<UdpDatagram> {
        // One extra byte converts platform-level UDP truncation into an explicit
        // over-limit error instead of silently accepting a prefix.
        let mut buffer = vec![0u8; self.max_datagram_size.saturating_add(1)];
        let (len, source) = self
            .socket
            .recv_from(&mut buffer)
            .map_err(|error| Gdl90Error::Io {
                context: "receive UDP datagram",
                details: error.to_string(),
            })?;
        if len > self.max_datagram_size {
            return Err(Gdl90Error::DatagramTooLarge {
                limit: self.max_datagram_size,
                actual: len,
            });
        }
        buffer.truncate(len);

        Ok(UdpDatagram {
            source,
            messages: decode_datagram(&buffer),
            bytes: buffer,
        })
    }
}

pub fn discover_foreflight_once(
    bind_addr: impl ToSocketAddrs,
    timeout: Duration,
) -> Result<(SocketAddr, ForeFlightDiscoveryAnnouncement)> {
    let socket = UdpSocket::bind(bind_addr).map_err(|error| Gdl90Error::Io {
        context: "bind ForeFlight discovery socket",
        details: error.to_string(),
    })?;
    socket
        .set_read_timeout(Some(timeout))
        .map_err(|error| Gdl90Error::Io {
            context: "set ForeFlight discovery timeout",
            details: error.to_string(),
        })?;

    let mut buffer = [0u8; DEFAULT_MAX_DATAGRAM_SIZE + 1];
    let (len, source) = socket
        .recv_from(&mut buffer)
        .map_err(|error| Gdl90Error::Io {
            context: "receive ForeFlight discovery datagram",
            details: error.to_string(),
        })?;
    if len > DEFAULT_MAX_DATAGRAM_SIZE {
        return Err(Gdl90Error::DatagramTooLarge {
            limit: DEFAULT_MAX_DATAGRAM_SIZE,
            actual: len,
        });
    }
    let text = std::str::from_utf8(&buffer[..len]).map_err(|_| Gdl90Error::Utf8 {
        field: "ForeFlight discovery datagram",
    })?;
    let announcement = ForeFlightDiscoveryAnnouncement::parse(text)?;
    Ok((source, announcement))
}

fn first_socket_addr(addrs: impl ToSocketAddrs, context: &'static str) -> Result<SocketAddr> {
    addrs
        .to_socket_addrs()
        .map_err(|error| Gdl90Error::Io {
            context,
            details: error.to_string(),
        })?
        .next()
        .ok_or(Gdl90Error::InvalidField {
            field: "socket address",
            details: "no address resolved".to_string(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::{FLAG_BYTE, encode_frame};
    use crate::message::{Heartbeat, HeartbeatStatus};

    fn heartbeat() -> Message {
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
            timestamp_seconds_since_midnight: 1,
            uplink_count: 0,
            basic_and_long_count: 0,
        })
    }

    #[test]
    fn parses_foreflight_discovery_json_example() {
        let json = r#"{
            "App":"ForeFlight",
            "GDL90":{
                "port":4000
            }
        }"#;

        let parsed = ForeFlightDiscoveryAnnouncement::parse(json).unwrap();
        assert_eq!(
            parsed,
            ForeFlightDiscoveryAnnouncement {
                app: "ForeFlight".to_string(),
                gdl90_port: 4000,
            }
        );
        assert!(parsed.is_foreflight());
    }

    #[test]
    fn derives_unicast_target_from_documented_discovery_source() {
        let announcement = ForeFlightDiscoveryAnnouncement {
            app: "ForeFlight".to_string(),
            gdl90_port: 4000,
        };
        let source: SocketAddr = "192.168.1.25:63093".parse().unwrap();
        assert_eq!(
            announcement.target_for_source(source),
            "192.168.1.25:4000".parse().unwrap()
        );
    }

    #[test]
    fn foreflight_sender_encodes_only_documented_message_sets() {
        let datagram = ForeFlightUdpSender::encode_messages(&[heartbeat()]).unwrap();
        assert!(!datagram.is_empty());

        let error = ForeFlightUdpSender::encode_messages(&[Message::Initialization(
            crate::message::Initialization {
                audio_test: false,
                audio_inhibit: false,
                cdti_ok: true,
                csa_audio_disable: false,
                csa_disable: false,
            },
        )])
        .unwrap_err();
        assert!(
            matches!(error, Gdl90Error::InvalidField { field, .. } if field == "ForeFlight supported message set")
        );
    }

    #[test]
    fn datagram_boundaries_do_not_share_decoder_state() {
        let first = decode_datagram(&[FLAG_BYTE, 0x00]);
        assert!(matches!(first.as_slice(), [Err(Gdl90Error::FrameTooShort)]));

        let frame = encode_frame(&heartbeat().encode().unwrap());
        let second = decode_datagram(&frame[1..]);
        assert!(second.is_empty());
    }

    #[test]
    fn receiver_detects_datagrams_larger_than_its_limit() {
        let mut receiver = UdpGdl90Receiver::bind("127.0.0.1:0").unwrap();
        receiver.set_max_datagram_size(8);
        let sender = UdpSocket::bind("127.0.0.1:0").unwrap();
        sender
            .send_to(&[0u8; 9], receiver.local_addr().unwrap())
            .unwrap();
        assert!(matches!(
            receiver.receive(),
            Err(Gdl90Error::DatagramTooLarge {
                limit: 8,
                actual: 9
            })
        ));
    }

    #[test]
    fn resolves_socket_address() {
        let addr = first_socket_addr("127.0.0.1:4000", "resolve address").unwrap();
        assert_eq!(addr.ip().to_string(), "127.0.0.1");
        assert_eq!(addr.port(), 4000);
    }

    #[test]
    fn rejects_missing_foreflight_fields() {
        let error = ForeFlightDiscoveryAnnouncement::parse(r#"{"App":"ForeFlight"}"#).unwrap_err();
        assert!(
            matches!(error, Gdl90Error::InvalidField { field, .. } if field == "ForeFlight discovery JSON")
        );
    }
}
''',
)

foreflight = read("src/foreflight.rs")
marker = "#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum GeometricAltitudeDatum"
prefix, suffix = foreflight.split(marker, 1)
new_prefix = r'''
use crate::error::{Gdl90Error, Result};
use crate::message::Message;
use crate::util::{decode_fixed_utf8, encode_fixed_utf8};
use std::net::IpAddr;
use std::time::Duration;

pub const FOREFLIGHT_MESSAGE_ID: u8 = 0x65;
pub const FOREFLIGHT_ID_MESSAGE_SUB_ID: u8 = 0x00;
pub const FOREFLIGHT_AHRS_MESSAGE_SUB_ID: u8 = 0x01;
pub const FOREFLIGHT_MAX_IP_PACKET_SIZE: usize = 1_500;
pub const FOREFLIGHT_IPV4_UDP_PAYLOAD_LIMIT: usize = 1_471;
pub const FOREFLIGHT_IPV6_UDP_PAYLOAD_LIMIT: usize = 1_451;
/// Conservative default that is safe for both IPv4 and IPv6 without extension headers.
pub const FOREFLIGHT_MAX_DATAGRAM_SIZE: usize = FOREFLIGHT_IPV6_UDP_PAYLOAD_LIMIT;
pub const FOREFLIGHT_AHRS_RATE_HZ: u8 = 5;
pub const FOREFLIGHT_AHRS_INTERVAL_MILLIS: u64 = 200;
pub const FOREFLIGHT_CONNECTIVITY_INTERVAL_SECONDS: u64 = 1;
pub const FOREFLIGHT_DISCOVERY_INTERVAL_SECONDS: u64 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ForeFlightCadenceProfile {
    pub ahrs_rate_hz: u8,
    pub ahrs_interval: Duration,
    pub connectivity_interval: Duration,
    pub discovery_interval: Duration,
}

pub fn cadence_profile() -> ForeFlightCadenceProfile {
    ForeFlightCadenceProfile {
        ahrs_rate_hz: FOREFLIGHT_AHRS_RATE_HZ,
        ahrs_interval: Duration::from_millis(FOREFLIGHT_AHRS_INTERVAL_MILLIS),
        connectivity_interval: Duration::from_secs(FOREFLIGHT_CONNECTIVITY_INTERVAL_SECONDS),
        discovery_interval: Duration::from_secs(FOREFLIGHT_DISCOVERY_INTERVAL_SECONDS),
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ForeFlightCadenceDue {
    pub ahrs: bool,
    pub connectivity: bool,
    pub discovery: bool,
}

/// Deterministic, monotonic-time scheduler. Missed periods are coalesced rather
/// than emitted as a burst, which keeps stale AHRS data from flooding a client.
#[derive(Debug, Clone)]
pub struct ForeFlightCadenceScheduler {
    profile: ForeFlightCadenceProfile,
    next_ahrs: Duration,
    next_connectivity: Duration,
    next_discovery: Duration,
}

impl ForeFlightCadenceScheduler {
    pub fn new(start: Duration) -> Self {
        Self::with_profile(start, cadence_profile())
    }

    pub fn with_profile(start: Duration, profile: ForeFlightCadenceProfile) -> Self {
        Self {
            profile,
            next_ahrs: start,
            next_connectivity: start,
            next_discovery: start,
        }
    }

    pub fn profile(&self) -> ForeFlightCadenceProfile {
        self.profile
    }

    pub fn poll(&mut self, now: Duration) -> ForeFlightCadenceDue {
        let due = ForeFlightCadenceDue {
            ahrs: now >= self.next_ahrs,
            connectivity: now >= self.next_connectivity,
            discovery: now >= self.next_discovery,
        };
        if due.ahrs {
            self.next_ahrs = now.saturating_add(self.profile.ahrs_interval);
        }
        if due.connectivity {
            self.next_connectivity = now.saturating_add(self.profile.connectivity_interval);
        }
        if due.discovery {
            self.next_discovery = now.saturating_add(self.profile.discovery_interval);
        }
        due
    }
}

pub fn is_supported_message(message: &Message) -> bool {
    matches!(
        message,
        Message::Heartbeat(_)
            | Message::UplinkData(_)
            | Message::OwnshipReport(_)
            | Message::OwnshipGeometricAltitude(_)
            | Message::TrafficReport(_)
            | Message::ForeFlightId(_)
            | Message::ForeFlightAhrs(_)
    )
}

pub fn has_connectivity_message(messages: &[Message]) -> bool {
    messages
        .iter()
        .any(|message| matches!(message, Message::Heartbeat(_) | Message::OwnshipReport(_)))
}

pub fn validate_message_set(messages: &[Message]) -> Result<()> {
    if messages.is_empty() {
        return Err(Gdl90Error::InvalidField {
            field: "ForeFlight message set",
            details: "must contain at least one message".to_string(),
        });
    }

    for message in messages {
        if !is_supported_message(message) {
            return Err(Gdl90Error::InvalidField {
                field: "ForeFlight supported message set",
                details: format!(
                    "{} is not part of the documented ForeFlight subset",
                    message.kind_name()
                ),
            });
        }
    }

    Ok(())
}

pub fn udp_payload_limit_for_ip(ip: IpAddr) -> usize {
    match ip {
        IpAddr::V4(_) => FOREFLIGHT_IPV4_UDP_PAYLOAD_LIMIT,
        IpAddr::V6(_) => FOREFLIGHT_IPV6_UDP_PAYLOAD_LIMIT,
    }
}

/// Encodes with the conservative IPv6-safe payload budget.
pub fn encode_datagram(messages: &[Message]) -> Result<Vec<u8>> {
    encode_datagram_with_limit(messages, FOREFLIGHT_MAX_DATAGRAM_SIZE)
}

pub fn encode_datagram_for_ip(messages: &[Message], ip: IpAddr) -> Result<Vec<u8>> {
    encode_datagram_with_limit(messages, udp_payload_limit_for_ip(ip))
}

pub fn encode_datagram_with_limit(messages: &[Message], limit: usize) -> Result<Vec<u8>> {
    validate_message_set(messages)?;
    if limit == 0 {
        return Err(Gdl90Error::InvalidField {
            field: "ForeFlight datagram size limit",
            details: "must be greater than zero".to_string(),
        });
    }

    let mut datagram = Vec::new();
    for message in messages {
        datagram.extend_from_slice(&message.encode_frame()?);
    }

    if datagram.len() > limit {
        return Err(Gdl90Error::InvalidField {
            field: "ForeFlight datagram size",
            details: format!(
                "encoded UDP payload is {} bytes, must be at most {limit} bytes so the complete IP packet remains below {FOREFLIGHT_MAX_IP_PACKET_SIZE} bytes",
                datagram.len()
            ),
        });
    }

    Ok(datagram)
}

'''
write("src/foreflight.rs", new_prefix + marker + suffix)

replace_once(
    "src/foreflight.rs",
    '''        assert_eq!(profile.ahrs_rate_hz, 5);\n        assert_eq!(profile.discovery_interval, Duration::from_secs(5));''',
    '''        assert_eq!(profile.ahrs_rate_hz, 5);\n        assert_eq!(profile.ahrs_interval, Duration::from_millis(200));\n        assert_eq!(profile.connectivity_interval, Duration::from_secs(1));\n        assert_eq!(profile.discovery_interval, Duration::from_secs(5));''',
)
replace_once(
    "src/foreflight.rs",
    '''    #[test]\n    fn foreflight_datagram_enforces_mtu() {\n        let oversized = vec![heartbeat(); 200];\n        let error = encode_datagram(&oversized).unwrap_err();\n        assert!(\n            matches!(error, Gdl90Error::InvalidField { field, .. } if field == "ForeFlight datagram size")\n        );\n    }''',
    '''    #[test]\n    fn foreflight_datagram_enforces_whole_packet_mtu() {\n        let oversized = vec![heartbeat(); 200];\n        let error = encode_datagram(&oversized).unwrap_err();\n        assert!(\n            matches!(error, Gdl90Error::InvalidField { field, .. } if field == "ForeFlight datagram size")\n        );\n        assert_eq!(FOREFLIGHT_IPV4_UDP_PAYLOAD_LIMIT + 20 + 8, 1_499);\n        assert_eq!(FOREFLIGHT_IPV6_UDP_PAYLOAD_LIMIT + 40 + 8, 1_499);\n    }\n\n    #[test]\n    fn cadence_scheduler_coalesces_missed_intervals() {\n        let mut scheduler = ForeFlightCadenceScheduler::new(Duration::ZERO);\n        assert_eq!(\n            scheduler.poll(Duration::ZERO),\n            ForeFlightCadenceDue {\n                ahrs: true,\n                connectivity: true,\n                discovery: true,\n            }\n        );\n        assert_eq!(\n            scheduler.poll(Duration::from_millis(199)),\n            ForeFlightCadenceDue::default()\n        );\n        assert!(scheduler.poll(Duration::from_millis(200)).ahrs);\n        let after_pause = scheduler.poll(Duration::from_secs(10));\n        assert!(after_pause.ahrs);\n        assert!(after_pause.connectivity);\n        assert!(after_pause.discovery);\n        assert_eq!(\n            scheduler.poll(Duration::from_secs(10)),\n            ForeFlightCadenceDue::default()\n        );\n    }''',
)

write(
    "src/bandwidth.rs",
    r'''
use crate::error::{Gdl90Error, Result};
use crate::message::Message;

#[derive(Debug, Clone, PartialEq)]
pub struct BandwidthConfig {
    pub baud_rate: u32,
    pub utilization_numerator: u32,
    pub utilization_denominator: u32,
    pub uplinks_per_second: usize,
    pub byte_budget_override: Option<usize>,
}

impl Default for BandwidthConfig {
    fn default() -> Self {
        Self {
            baud_rate: 38_400,
            utilization_numerator: 90,
            utilization_denominator: 100,
            uplinks_per_second: 4,
            byte_budget_override: None,
        }
    }
}

impl BandwidthConfig {
    pub fn validate(&self) -> Result<()> {
        if self.baud_rate == 0 {
            return Err(Gdl90Error::InvalidField {
                field: "bandwidth baud rate",
                details: "must be greater than zero".to_string(),
            });
        }
        if self.utilization_denominator == 0 {
            return Err(Gdl90Error::InvalidField {
                field: "bandwidth utilization denominator",
                details: "must be greater than zero".to_string(),
            });
        }
        if self.utilization_numerator > self.utilization_denominator {
            return Err(Gdl90Error::InvalidField {
                field: "bandwidth utilization",
                details: "numerator must not exceed denominator".to_string(),
            });
        }
        if self.uplinks_per_second > 4 {
            return Err(Gdl90Error::InvalidField {
                field: "primary uplinks per second",
                details: "Garmin Rev A permits a configured value in the range 0..=4".to_string(),
            });
        }
        if self.byte_budget_override == Some(0) {
            return Err(Gdl90Error::InvalidField {
                field: "bandwidth byte budget override",
                details: "must be greater than zero when present".to_string(),
            });
        }
        Ok(())
    }

    pub fn byte_budget_per_second(&self) -> Result<usize> {
        self.validate()?;
        if let Some(budget) = self.byte_budget_override {
            return Ok(budget);
        }

        let numerator = u64::from(self.baud_rate)
            .checked_mul(u64::from(self.utilization_numerator))
            .ok_or(Gdl90Error::InvalidField {
                field: "bandwidth byte budget",
                details: "calculation overflowed".to_string(),
            })?;
        let denominator = 10u64
            .checked_mul(u64::from(self.utilization_denominator))
            .ok_or(Gdl90Error::InvalidField {
                field: "bandwidth byte budget",
                details: "calculation overflowed".to_string(),
            })?;
        usize::try_from(numerator / denominator).map_err(|_| Gdl90Error::InvalidField {
            field: "bandwidth byte budget",
            details: "calculated budget does not fit in usize".to_string(),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TrafficCandidate {
    pub range_nm: f64,
    pub message: Message,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UplinkCandidate {
    pub station_range_nm: f64,
    pub time_slot: u8,
    pub message: Message,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScheduleInputs {
    pub heartbeat: Message,
    pub ownship: Message,
    pub alert_traffic: Vec<TrafficCandidate>,
    pub uplinks: Vec<UplinkCandidate>,
    pub proximate_traffic: Vec<TrafficCandidate>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduledStage {
    Heartbeat,
    Ownship,
    AlertTraffic,
    PrimaryUplink,
    ProximateTraffic,
    SecondaryUplink,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScheduledMessage {
    pub stage: ScheduledStage,
    pub size_bytes: usize,
    pub message: Message,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScheduleResult {
    pub byte_budget: usize,
    pub used_bytes: usize,
    pub selected: Vec<ScheduledMessage>,
    pub dropped_alert_traffic: usize,
    pub dropped_proximate_traffic: usize,
    pub dropped_uplinks: usize,
    pub over_budget_due_to_mandatory_messages: bool,
}

#[derive(Debug, Clone)]
pub struct BandwidthManager {
    config: BandwidthConfig,
}

impl BandwidthManager {
    pub fn new(config: BandwidthConfig) -> Result<Self> {
        config.validate()?;
        Ok(Self { config })
    }

    pub fn config(&self) -> &BandwidthConfig {
        &self.config
    }

    pub fn schedule(&self, mut inputs: ScheduleInputs) -> Result<ScheduleResult> {
        let byte_budget = self.config.byte_budget_per_second()?;
        let mut selected = Vec::new();
        let mut used_bytes = 0usize;

        push_mandatory(
            ScheduledStage::Heartbeat,
            inputs.heartbeat,
            &mut selected,
            &mut used_bytes,
        )?;
        push_mandatory(
            ScheduledStage::Ownship,
            inputs.ownship,
            &mut selected,
            &mut used_bytes,
        )?;

        let over_budget_due_to_mandatory_messages = used_bytes > byte_budget;

        inputs
            .alert_traffic
            .sort_by(|left, right| left.range_nm.total_cmp(&right.range_nm));
        inputs
            .proximate_traffic
            .sort_by(|left, right| left.range_nm.total_cmp(&right.range_nm));

        let mut eligible_uplinks = Vec::new();
        let mut invalid_uplink_count = 0usize;
        for candidate in inputs.uplinks {
            let application_data_valid = match &candidate.message {
                Message::UplinkData(message) => {
                    message.payload.decoded_header()?.application_data_valid
                }
                _ => {
                    return Err(Gdl90Error::InvalidField {
                        field: "uplink candidate message",
                        details: "must contain a GDL90 Uplink Data message".to_string(),
                    });
                }
            };
            if application_data_valid {
                eligible_uplinks.push(candidate);
            } else {
                invalid_uplink_count += 1;
            }
        }
        eligible_uplinks.sort_by(|left, right| {
            left.station_range_nm
                .total_cmp(&right.station_range_nm)
                .then(left.time_slot.cmp(&right.time_slot))
        });

        let primary_limit = self.config.uplinks_per_second.min(eligible_uplinks.len());
        let mut primary_uplinks = eligible_uplinks.drain(..primary_limit).collect::<Vec<_>>();
        let mut secondary_uplinks = eligible_uplinks;

        let dropped_alert_traffic = schedule_traffic_group(
            ScheduledStage::AlertTraffic,
            inputs.alert_traffic,
            byte_budget,
            &mut used_bytes,
            &mut selected,
        )?;
        let dropped_primary_uplinks = schedule_uplink_group(
            ScheduledStage::PrimaryUplink,
            primary_uplinks.as_mut_slice(),
            byte_budget,
            &mut used_bytes,
            &mut selected,
        )?;
        let dropped_proximate_traffic = schedule_traffic_group(
            ScheduledStage::ProximateTraffic,
            inputs.proximate_traffic,
            byte_budget,
            &mut used_bytes,
            &mut selected,
        )?;
        let dropped_secondary_uplinks = schedule_uplink_group(
            ScheduledStage::SecondaryUplink,
            secondary_uplinks.as_mut_slice(),
            byte_budget,
            &mut used_bytes,
            &mut selected,
        )?;

        Ok(ScheduleResult {
            byte_budget,
            used_bytes,
            selected,
            dropped_alert_traffic,
            dropped_proximate_traffic,
            dropped_uplinks: invalid_uplink_count
                + dropped_primary_uplinks
                + dropped_secondary_uplinks,
            over_budget_due_to_mandatory_messages,
        })
    }
}

fn push_mandatory(
    stage: ScheduledStage,
    message: Message,
    selected: &mut Vec<ScheduledMessage>,
    used_bytes: &mut usize,
) -> Result<()> {
    let size_bytes = message.encode_frame()?.len();
    *used_bytes = used_bytes
        .checked_add(size_bytes)
        .ok_or(Gdl90Error::InvalidField {
            field: "bandwidth used bytes",
            details: "counter overflowed".to_string(),
        })?;
    selected.push(ScheduledMessage {
        stage,
        size_bytes,
        message,
    });
    Ok(())
}

fn schedule_traffic_group(
    stage: ScheduledStage,
    candidates: Vec<TrafficCandidate>,
    byte_budget: usize,
    used_bytes: &mut usize,
    selected: &mut Vec<ScheduledMessage>,
) -> Result<usize> {
    let mut dropped = 0usize;
    for candidate in candidates {
        let size_bytes = candidate.message.encode_frame()?.len();
        if let Some(total) = used_bytes.checked_add(size_bytes)
            && total <= byte_budget
        {
            *used_bytes = total;
            selected.push(ScheduledMessage {
                stage,
                size_bytes,
                message: candidate.message,
            });
        } else {
            dropped += 1;
        }
    }
    Ok(dropped)
}

fn schedule_uplink_group(
    stage: ScheduledStage,
    candidates: &mut [UplinkCandidate],
    byte_budget: usize,
    used_bytes: &mut usize,
    selected: &mut Vec<ScheduledMessage>,
) -> Result<usize> {
    let mut dropped = 0usize;
    for candidate in candidates {
        let size_bytes = candidate.message.encode_frame()?.len();
        if let Some(total) = used_bytes.checked_add(size_bytes)
            && total <= byte_budget
        {
            *used_bytes = total;
            selected.push(ScheduledMessage {
                stage,
                size_bytes,
                message: candidate.message.clone(),
            });
        } else {
            dropped += 1;
        }
    }
    Ok(dropped)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::UplinkData;
    use crate::uplink::{APPLICATION_DATA_LEN, UatUplinkHeader, UatUplinkPayload};

    fn dummy_message(id: u8, payload_len: usize) -> Message {
        Message::Unknown {
            message_id: id,
            data: vec![0u8; payload_len],
        }
    }

    fn uplink_message(application_data_valid: bool) -> Message {
        let header = UatUplinkHeader {
            position_valid: false,
            latitude_deg: 0.0,
            longitude_deg: 0.0,
            utc_coupled: false,
            application_data_valid,
            slot_id: 0,
            tisb_site_id: 0,
        }
        .encode()
        .unwrap();
        Message::UplinkData(UplinkData {
            time_of_reception: None,
            payload: UatUplinkPayload {
                header,
                application_data: [0; APPLICATION_DATA_LEN],
            },
        })
    }

    #[test]
    fn schedules_in_documented_stage_order() {
        let manager = BandwidthManager::new(BandwidthConfig {
            byte_budget_override: Some(470),
            uplinks_per_second: 1,
            ..BandwidthConfig::default()
        })
        .unwrap();

        let result = manager
            .schedule(ScheduleInputs {
                heartbeat: dummy_message(1, 0),
                ownship: dummy_message(2, 0),
                alert_traffic: vec![
                    TrafficCandidate {
                        range_nm: 5.0,
                        message: dummy_message(3, 0),
                    },
                    TrafficCandidate {
                        range_nm: 1.0,
                        message: dummy_message(4, 0),
                    },
                ],
                uplinks: vec![
                    UplinkCandidate {
                        station_range_nm: 10.0,
                        time_slot: 4,
                        message: uplink_message(true),
                    },
                    UplinkCandidate {
                        station_range_nm: 2.0,
                        time_slot: 1,
                        message: uplink_message(true),
                    },
                ],
                proximate_traffic: vec![TrafficCandidate {
                    range_nm: 3.0,
                    message: dummy_message(8, 0),
                }],
            })
            .unwrap();

        let stages_and_ids = result
            .selected
            .iter()
            .map(|scheduled| (scheduled.stage, scheduled.message.message_id()))
            .collect::<Vec<_>>();

        assert_eq!(
            stages_and_ids,
            vec![
                (ScheduledStage::Heartbeat, 1),
                (ScheduledStage::Ownship, 2),
                (ScheduledStage::AlertTraffic, 4),
                (ScheduledStage::AlertTraffic, 3),
                (ScheduledStage::PrimaryUplink, 7),
                (ScheduledStage::ProximateTraffic, 8),
            ]
        );
        assert_eq!(result.dropped_uplinks, 1);
        assert_eq!(result.dropped_alert_traffic, 0);
        assert_eq!(result.dropped_proximate_traffic, 0);
    }

    #[test]
    fn derives_application_data_validity_from_the_uplink_header() {
        let manager = BandwidthManager::new(BandwidthConfig {
            byte_budget_override: Some(500),
            uplinks_per_second: 4,
            ..BandwidthConfig::default()
        })
        .unwrap();

        let result = manager
            .schedule(ScheduleInputs {
                heartbeat: dummy_message(1, 0),
                ownship: dummy_message(2, 0),
                alert_traffic: Vec::new(),
                uplinks: vec![
                    UplinkCandidate {
                        station_range_nm: 1.0,
                        time_slot: 0,
                        message: uplink_message(false),
                    },
                    UplinkCandidate {
                        station_range_nm: 1.0,
                        time_slot: 1,
                        message: uplink_message(true),
                    },
                ],
                proximate_traffic: Vec::new(),
            })
            .unwrap();

        let ids = result
            .selected
            .iter()
            .map(|scheduled| scheduled.message.message_id())
            .collect::<Vec<_>>();
        assert_eq!(ids, vec![1, 2, 7]);
        assert_eq!(result.dropped_uplinks, 1);
    }

    #[test]
    fn rejects_invalid_configuration_instead_of_panicking() {
        let error = BandwidthManager::new(BandwidthConfig {
            utilization_denominator: 0,
            ..BandwidthConfig::default()
        })
        .unwrap_err();
        assert!(matches!(
            error,
            Gdl90Error::InvalidField {
                field: "bandwidth utilization denominator",
                ..
            }
        ));

        assert!(BandwidthManager::new(BandwidthConfig {
            uplinks_per_second: 5,
            ..BandwidthConfig::default()
        })
        .is_err());
    }

    #[test]
    fn rejects_non_uplink_candidates() {
        let manager = BandwidthManager::new(BandwidthConfig {
            byte_budget_override: Some(500),
            ..BandwidthConfig::default()
        })
        .unwrap();
        let error = manager
            .schedule(ScheduleInputs {
                heartbeat: dummy_message(1, 0),
                ownship: dummy_message(2, 0),
                alert_traffic: Vec::new(),
                uplinks: vec![UplinkCandidate {
                    station_range_nm: 1.0,
                    time_slot: 0,
                    message: dummy_message(3, 0),
                }],
                proximate_traffic: Vec::new(),
            })
            .unwrap_err();
        assert!(matches!(
            error,
            Gdl90Error::InvalidField {
                field: "uplink candidate message",
                ..
            }
        ));
    }

    #[test]
    fn marks_over_budget_if_mandatory_messages_alone_exceed_budget() {
        let manager = BandwidthManager::new(BandwidthConfig {
            byte_budget_override: Some(8),
            ..BandwidthConfig::default()
        })
        .unwrap();

        let result = manager
            .schedule(ScheduleInputs {
                heartbeat: dummy_message(1, 0),
                ownship: dummy_message(2, 0),
                alert_traffic: Vec::new(),
                uplinks: Vec::new(),
                proximate_traffic: Vec::new(),
            })
            .unwrap();

        assert!(result.over_budget_due_to_mandatory_messages);
        assert_eq!(result.selected.len(), 2);
    }
}
''',
)
