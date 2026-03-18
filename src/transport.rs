use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::time::Duration;

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
        let app = extract_json_string_field(json, "App").ok_or(Gdl90Error::InvalidField {
            field: "ForeFlight discovery JSON",
            details: "missing App field".to_string(),
        })?;
        let gdl90_object =
            extract_json_object_field(json, "GDL90").ok_or(Gdl90Error::InvalidField {
                field: "ForeFlight discovery JSON",
                details: "missing GDL90 object".to_string(),
            })?;
        let gdl90_port =
            extract_json_u16_field(gdl90_object, "port").ok_or(Gdl90Error::InvalidField {
                field: "ForeFlight discovery JSON",
                details: "missing GDL90.port field".to_string(),
            })?;

        Ok(Self { app, gdl90_port })
    }

    pub fn is_foreflight(&self) -> bool {
        self.app == "ForeFlight"
    }

    pub fn target_for_source(&self, source: SocketAddr) -> SocketAddr {
        SocketAddr::new(source.ip(), self.gdl90_port)
    }
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

    pub fn encode_messages(messages: &[Message]) -> Result<Vec<u8>> {
        foreflight::encode_datagram(messages)
    }

    pub fn send_message(&self, message: &Message) -> Result<usize> {
        self.send_messages(std::slice::from_ref(message))
    }

    pub fn send_messages(&self, messages: &[Message]) -> Result<usize> {
        let datagram = Self::encode_messages(messages)?;
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
    decoder: FrameMessageDecoder,
    max_datagram_size: usize,
}

#[derive(Debug)]
pub struct UdpDatagram {
    pub source: SocketAddr,
    pub bytes: Vec<u8>,
    pub messages: Vec<Result<Message>>,
}

impl UdpGdl90Receiver {
    pub fn bind(bind_addr: impl ToSocketAddrs) -> Result<Self> {
        let socket = UdpSocket::bind(bind_addr).map_err(|error| Gdl90Error::Io {
            context: "bind UDP receiver socket",
            details: error.to_string(),
        })?;
        Ok(Self {
            socket,
            decoder: FrameMessageDecoder::new(),
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
        let mut buffer = vec![0u8; self.max_datagram_size];
        let (len, source) = self
            .socket
            .recv_from(&mut buffer)
            .map_err(|error| Gdl90Error::Io {
                context: "receive UDP datagram",
                details: error.to_string(),
            })?;
        buffer.truncate(len);

        Ok(UdpDatagram {
            source,
            messages: self.decoder.push(&buffer),
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

    let mut buffer = [0u8; DEFAULT_MAX_DATAGRAM_SIZE];
    let (len, source) = socket
        .recv_from(&mut buffer)
        .map_err(|error| Gdl90Error::Io {
            context: "receive ForeFlight discovery datagram",
            details: error.to_string(),
        })?;
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

fn extract_json_string_field<'a>(json: &'a str, key: &str) -> Option<String> {
    let value = extract_json_value_region(json, key)?;
    let trimmed = value.trim_start();
    let start = trimmed.find('"')?;
    let rest = &trimmed[start + 1..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn extract_json_object_field<'a>(json: &'a str, key: &str) -> Option<&'a str> {
    let value = extract_json_value_region(json, key)?;
    let trimmed = value.trim_start();
    let mut chars = trimmed.char_indices();
    let (_, first) = chars.next()?;
    if first != '{' {
        return None;
    }

    let mut depth = 0usize;
    for (index, ch) in trimmed.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&trimmed[..=index]);
                }
            }
            _ => {}
        }
    }
    None
}

fn extract_json_u16_field(json: &str, key: &str) -> Option<u16> {
    let value = extract_json_value_region(json, key)?;
    let trimmed = value.trim_start();
    let digits: String = trimmed
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect();
    if digits.is_empty() {
        return None;
    }
    digits.parse().ok()
}

fn extract_json_value_region<'a>(json: &'a str, key: &str) -> Option<&'a str> {
    let pattern = format!("\"{key}\"");
    let key_start = json.find(&pattern)?;
    let after_key = &json[key_start + pattern.len()..];
    let colon_index = after_key.find(':')?;
    Some(&after_key[colon_index + 1..])
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn parses_foreflight_discovery_json() {
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
    fn derives_unicast_target_from_discovery_source() {
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
