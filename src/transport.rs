use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::time::Duration;

use serde::Deserialize;

use crate::error::{Gdl90Error, Result};
use crate::foreflight;
use crate::message::Message;

pub const FOREFLIGHT_DISCOVERY_PORT: u16 = 63_093;
pub const FOREFLIGHT_GDL90_PORT: u16 = 4_000;
pub const DEFAULT_MAX_DATAGRAM_SIZE: usize = 2_048;
/// Conservative common IPv4/IPv6 UDP payload ceiling (no IP jumbograms).
pub const MAX_UDP_PAYLOAD_SIZE: usize = 65_507;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForeFlightDiscoveryAnnouncement {
    pub app: String,
    pub gdl90_port: u16,
}

impl ForeFlightDiscoveryAnnouncement {
    pub fn parse(json: &str) -> Result<Self> {
        if json.len() > DEFAULT_MAX_DATAGRAM_SIZE {
            return Err(Gdl90Error::DatagramTooLarge {
                limit: DEFAULT_MAX_DATAGRAM_SIZE,
                actual: json.len(),
            });
        }
        let envelope: ForeFlightDiscoveryEnvelope =
            serde_json::from_str(json).map_err(|error| Gdl90Error::InvalidField {
                field: "ForeFlight discovery JSON",
                details: error.to_string(),
            })?;

        if envelope.gdl90.port == 0 {
            return Err(Gdl90Error::InvalidField {
                field: "ForeFlight discovery port",
                details: "port zero is not a destination".into(),
            });
        }
        Ok(Self {
            app: envelope.app,
            gdl90_port: envelope.gdl90.port,
        })
    }

    pub fn is_foreflight(&self) -> bool {
        self.app == "ForeFlight"
    }

    pub fn target_for_source(&self, source: SocketAddr) -> SocketAddr {
        let mut target = source;
        target.set_port(self.gdl90_port);
        target
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
        let socket = UdpSocket::bind(bind_addr)
            .map_err(|error| Gdl90Error::io("bind UDP sender socket", error))?;
        let ipv4 = socket
            .local_addr()
            .map_err(|error| Gdl90Error::io("read UDP socket family", error))?
            .is_ipv4();
        let target = target
            .to_socket_addrs()
            .map_err(|error| Gdl90Error::io("resolve UDP target address", error))?
            .find(|address| address.is_ipv4() == ipv4 && address.port() != 0)
            .ok_or(Gdl90Error::InvalidField {
                field: "UDP target",
                details: "no nonzero-port address matches the bound socket family".into(),
            })?;
        Ok(Self { socket, target })
    }

    pub fn local_addr(&self) -> Result<SocketAddr> {
        self.socket
            .local_addr()
            .map_err(|error| Gdl90Error::io("read UDP sender local address", error))
    }

    pub fn socket(&self) -> &UdpSocket {
        &self.socket
    }

    pub fn target(&self) -> SocketAddr {
        self.target
    }

    pub fn send_message(&self, message: &Message) -> Result<usize> {
        self.send_messages(std::slice::from_ref(message))
    }

    pub fn send_messages(&self, messages: &[Message]) -> Result<usize> {
        if messages.is_empty() {
            return Err(Gdl90Error::InvalidField {
                field: "UDP messages",
                details: "must not be empty".into(),
            });
        }
        let mut datagram = Vec::new();
        for message in messages {
            let frame = message.encode_frame_with_limit(MAX_UDP_PAYLOAD_SIZE - datagram.len())?;
            datagram.extend_from_slice(&frame);
        }
        self.send_frame(&datagram)
    }

    pub fn send_frame(&self, frame: &[u8]) -> Result<usize> {
        if frame.len() > MAX_UDP_PAYLOAD_SIZE {
            return Err(Gdl90Error::DatagramTooLarge {
                limit: MAX_UDP_PAYLOAD_SIZE,
                actual: frame.len(),
            });
        }
        self.socket
            .send_to(frame, self.target)
            .map_err(|error| Gdl90Error::io("send UDP datagram", error))
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
    crate::frame::decode_datagram_frames(bytes)
        .into_iter()
        .map(|result| result.and_then(|clear| Message::decode(&clear)))
        .collect()
}

impl UdpGdl90Receiver {
    pub fn bind(bind_addr: impl ToSocketAddrs) -> Result<Self> {
        let socket = UdpSocket::bind(bind_addr)
            .map_err(|error| Gdl90Error::io("bind UDP receiver socket", error))?;
        Ok(Self {
            socket,
            max_datagram_size: DEFAULT_MAX_DATAGRAM_SIZE,
        })
    }

    pub fn local_addr(&self) -> Result<SocketAddr> {
        self.socket
            .local_addr()
            .map_err(|error| Gdl90Error::io("read UDP receiver local address", error))
    }

    pub fn socket(&self) -> &UdpSocket {
        &self.socket
    }

    pub fn set_read_timeout(&self, timeout: Option<Duration>) -> Result<()> {
        self.socket
            .set_read_timeout(timeout)
            .map_err(|error| Gdl90Error::io("set UDP receiver read timeout", error))
    }

    /// Clamp to 1..=65507: configuration cannot request an impossible allocation.
    pub fn set_max_datagram_size(&mut self, size: usize) {
        self.max_datagram_size = size.clamp(1, MAX_UDP_PAYLOAD_SIZE);
    }

    pub fn max_datagram_size(&self) -> usize {
        self.max_datagram_size
    }

    pub fn receive(&mut self) -> Result<UdpDatagram> {
        // One extra byte converts platform-level UDP truncation into an explicit
        // over-limit error instead of silently accepting a prefix.
        let mut buffer = vec![0u8; self.max_datagram_size.saturating_add(1)];
        let (len, source) = self.socket.recv_from(&mut buffer).map_err(|error| {
            if cfg!(windows) && error.raw_os_error() == Some(10040) {
                // WSAEMSGSIZE: Windows reports truncation as an error instead of a length.
                Gdl90Error::DatagramTooLarge {
                    limit: self.max_datagram_size,
                    actual: self.max_datagram_size + 1,
                }
            } else {
                Gdl90Error::io("receive UDP datagram", error)
            }
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
    let socket = UdpSocket::bind(bind_addr)
        .map_err(|error| Gdl90Error::io("bind ForeFlight discovery socket", error))?;
    socket
        .set_read_timeout(Some(timeout))
        .map_err(|error| Gdl90Error::io("set ForeFlight discovery timeout", error))?;

    let mut buffer = [0u8; DEFAULT_MAX_DATAGRAM_SIZE + 1];
    let (len, source) = socket
        .recv_from(&mut buffer)
        .map_err(|error| Gdl90Error::io("receive ForeFlight discovery datagram", error))?;
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
        assert!(second.iter().all(Result::is_err));
        assert!(!second.is_empty());
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
    fn sender_selects_a_compatible_address_and_rejects_zero_port() {
        let sender = UdpGdl90Sender::bind("127.0.0.1:0", "127.0.0.1:4000").unwrap();
        assert_eq!(sender.target(), "127.0.0.1:4000".parse().unwrap());
        assert!(UdpGdl90Sender::bind("127.0.0.1:0", "[::1]:4000").is_err());
        assert!(UdpGdl90Sender::bind("127.0.0.1:0", "127.0.0.1:0").is_err());
    }

    #[test]
    fn rejects_missing_foreflight_fields() {
        let error = ForeFlightDiscoveryAnnouncement::parse(r#"{"App":"ForeFlight"}"#).unwrap_err();
        assert!(
            matches!(error, Gdl90Error::InvalidField { field, .. } if field == "ForeFlight discovery JSON")
        );
    }
}
