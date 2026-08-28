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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeometricAltitudeDatum {
    Wgs84Ellipsoid,
    MeanSeaLevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InternetPolicy {
    Unrestricted,
    Expensive,
    Disallowed,
    Reserved(u8),
}

impl InternetPolicy {
    fn from_raw(raw: u8) -> Self {
        match raw {
            0 => Self::Unrestricted,
            1 => Self::Expensive,
            2 => Self::Disallowed,
            other => Self::Reserved(other),
        }
    }

    fn raw(self) -> u8 {
        match self {
            Self::Unrestricted => 0,
            Self::Expensive => 1,
            Self::Disallowed => 2,
            Self::Reserved(bits) => bits & 0x03,
        }
    }

    fn validate(self) -> Result<()> {
        if matches!(self, Self::Reserved(_)) {
            return Err(Gdl90Error::InvalidField {
                field: "ForeFlight internet policy",
                details: "reserved policy value is not valid for transmitted messages".to_string(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ForeFlightCapabilities {
    pub geometric_altitude_datum: GeometricAltitudeDatum,
    pub internet_policy: InternetPolicy,
    pub reserved_bits: u32,
}

impl ForeFlightCapabilities {
    pub fn from_raw(raw: u32) -> Self {
        Self {
            geometric_altitude_datum: if (raw & 0x01) == 0 {
                GeometricAltitudeDatum::Wgs84Ellipsoid
            } else {
                GeometricAltitudeDatum::MeanSeaLevel
            },
            internet_policy: InternetPolicy::from_raw(((raw >> 1) & 0x03) as u8),
            reserved_bits: raw & !0x07,
        }
    }

    pub fn raw(self) -> u32 {
        let datum = match self.geometric_altitude_datum {
            GeometricAltitudeDatum::Wgs84Ellipsoid => 0u32,
            GeometricAltitudeDatum::MeanSeaLevel => 1u32,
        };
        datum | ((self.internet_policy.raw() as u32) << 1) | (self.reserved_bits & !0x07)
    }

    pub fn validate(&self) -> Result<()> {
        self.internet_policy.validate()?;
        if self.reserved_bits != 0 {
            return Err(Gdl90Error::InvalidField {
                field: "ForeFlight capabilities reserved bits",
                details: "reserved bits must be zero".to_string(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForeFlightIdMessage {
    pub version: u8,
    pub device_serial_number: Option<u64>,
    pub device_name: String,
    pub device_long_name: String,
    pub capabilities: ForeFlightCapabilities,
}

impl ForeFlightIdMessage {
    pub const LEN: usize = 39;

    pub fn validate(&self) -> Result<()> {
        if self.version != 1 {
            return Err(Gdl90Error::InvalidField {
                field: "ForeFlight ID version",
                details: format!("{} is not the documented version 1", self.version),
            });
        }
        self.capabilities.validate()
    }

    pub fn decode(payload: &[u8]) -> Result<Self> {
        if payload.len() != Self::LEN {
            return Err(Gdl90Error::InvalidLength {
                context: "ForeFlight ID message",
                expected: "39 bytes",
                actual: payload.len(),
            });
        }
        if payload[0] != FOREFLIGHT_MESSAGE_ID || payload[1] != FOREFLIGHT_ID_MESSAGE_SUB_ID {
            return Err(Gdl90Error::InvalidField {
                field: "ForeFlight ID header",
                details: "unexpected message id or sub-id".to_string(),
            });
        }

        let version = payload[2];
        let serial = u64::from_be_bytes(payload[3..11].try_into().unwrap());
        let device_serial_number = if serial == u64::MAX {
            None
        } else {
            Some(serial)
        };
        let device_name = decode_fixed_utf8(&payload[11..19], "device name")?;
        let device_long_name = decode_fixed_utf8(&payload[19..35], "device long name")?;
        let capabilities = ForeFlightCapabilities::from_raw(u32::from_be_bytes(
            payload[35..39].try_into().unwrap(),
        ));
        let message = Self {
            version,
            device_serial_number,
            device_name,
            device_long_name,
            capabilities,
        };
        message.validate()?;
        Ok(message)
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        self.validate()?;

        let mut out = Vec::with_capacity(Self::LEN);
        out.push(FOREFLIGHT_MESSAGE_ID);
        out.push(FOREFLIGHT_ID_MESSAGE_SUB_ID);
        out.push(self.version);
        out.extend_from_slice(&self.device_serial_number.unwrap_or(u64::MAX).to_be_bytes());
        out.extend_from_slice(&encode_fixed_utf8::<8>(&self.device_name, "device name")?);
        out.extend_from_slice(&encode_fixed_utf8::<16>(
            &self.device_long_name,
            "device long name",
        )?);
        out.extend_from_slice(&self.capabilities.raw().to_be_bytes());
        Ok(out)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeadingType {
    True,
    Magnetic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Heading {
    pub heading_type: HeadingType,
    pub tenths_degrees: i16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForeFlightAhrsMessage {
    pub roll_tenths_degrees: Option<i16>,
    pub pitch_tenths_degrees: Option<i16>,
    pub heading: Option<Heading>,
    pub indicated_airspeed_knots: Option<u16>,
    pub true_airspeed_knots: Option<u16>,
}

impl ForeFlightAhrsMessage {
    pub const LEN: usize = 12;

    pub fn decode(payload: &[u8]) -> Result<Self> {
        if payload.len() != Self::LEN {
            return Err(Gdl90Error::InvalidLength {
                context: "ForeFlight AHRS message",
                expected: "12 bytes",
                actual: payload.len(),
            });
        }
        if payload[0] != FOREFLIGHT_MESSAGE_ID || payload[1] != FOREFLIGHT_AHRS_MESSAGE_SUB_ID {
            return Err(Gdl90Error::InvalidField {
                field: "ForeFlight AHRS header",
                details: "unexpected message id or sub-id".to_string(),
            });
        }

        let roll = decode_optional_signed_range(
            i16::from_be_bytes([payload[2], payload[3]]),
            0x7FFF,
            -1800,
            1800,
            "AHRS roll",
        )?;
        let pitch = decode_optional_signed_range(
            i16::from_be_bytes([payload[4], payload[5]]),
            0x7FFF,
            -1800,
            1800,
            "AHRS pitch",
        )?;

        let raw_heading = u16::from_be_bytes([payload[6], payload[7]]);
        let heading = if raw_heading == 0xFFFF {
            None
        } else {
            let heading_type = if (raw_heading & 0x8000) == 0 {
                HeadingType::True
            } else {
                HeadingType::Magnetic
            };
            let value = (raw_heading & 0x7FFF) as i16;
            if !(0..=3600).contains(&value) {
                return Err(Gdl90Error::InvalidField {
                    field: "AHRS heading",
                    details: format!("{value} is outside [0, 3600]"),
                });
            }
            Some(Heading {
                heading_type,
                tenths_degrees: value,
            })
        };

        Ok(Self {
            roll_tenths_degrees: roll,
            pitch_tenths_degrees: pitch,
            heading,
            indicated_airspeed_knots: decode_optional_u16(u16::from_be_bytes([
                payload[8], payload[9],
            ])),
            true_airspeed_knots: decode_optional_u16(u16::from_be_bytes([
                payload[10],
                payload[11],
            ])),
        })
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        let mut out = Vec::with_capacity(Self::LEN);
        out.push(FOREFLIGHT_MESSAGE_ID);
        out.push(FOREFLIGHT_AHRS_MESSAGE_SUB_ID);
        out.extend_from_slice(&encode_optional_signed_range(
            self.roll_tenths_degrees,
            0x7FFF,
            -1800,
            1800,
            "AHRS roll",
        )?);
        out.extend_from_slice(&encode_optional_signed_range(
            self.pitch_tenths_degrees,
            0x7FFF,
            -1800,
            1800,
            "AHRS pitch",
        )?);
        let heading = if let Some(heading) = self.heading {
            if !(0..=3600).contains(&heading.tenths_degrees) {
                return Err(Gdl90Error::InvalidField {
                    field: "AHRS heading",
                    details: format!("{} is outside [0, 3600]", heading.tenths_degrees),
                });
            }
            let type_bit = match heading.heading_type {
                HeadingType::True => 0u16,
                HeadingType::Magnetic => 0x8000,
            };
            type_bit | (heading.tenths_degrees as u16 & 0x7FFF)
        } else {
            0xFFFF
        };
        out.extend_from_slice(&heading.to_be_bytes());
        out.extend_from_slice(
            &self
                .indicated_airspeed_knots
                .unwrap_or(0xFFFF)
                .to_be_bytes(),
        );
        out.extend_from_slice(&self.true_airspeed_knots.unwrap_or(0xFFFF).to_be_bytes());
        Ok(out)
    }
}

fn decode_optional_u16(value: u16) -> Option<u16> {
    if value == 0xFFFF { None } else { Some(value) }
}

fn decode_optional_signed_range(
    value: i16,
    invalid: i16,
    min: i16,
    max: i16,
    field: &'static str,
) -> Result<Option<i16>> {
    if value == invalid {
        return Ok(None);
    }
    if !(min..=max).contains(&value) {
        return Err(Gdl90Error::InvalidField {
            field,
            details: format!("{value} is outside [{min}, {max}]"),
        });
    }
    Ok(Some(value))
}

fn encode_optional_signed_range(
    value: Option<i16>,
    invalid: i16,
    min: i16,
    max: i16,
    field: &'static str,
) -> Result<[u8; 2]> {
    let raw = if let Some(value) = value {
        if !(min..=max).contains(&value) {
            return Err(Gdl90Error::InvalidField {
                field,
                details: format!("{value} is outside [{min}, {max}]"),
            });
        }
        value
    } else {
        invalid
    };
    Ok(raw.to_be_bytes())
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
    fn cadence_profile_matches_spec() {
        let profile = cadence_profile();
        assert_eq!(profile.ahrs_rate_hz, 5);
        assert_eq!(profile.ahrs_interval, Duration::from_millis(200));
        assert_eq!(profile.connectivity_interval, Duration::from_secs(1));
        assert_eq!(profile.discovery_interval, Duration::from_secs(5));
    }

    #[test]
    fn foreflight_message_set_allows_supported_non_connectivity_messages() {
        validate_message_set(&[Message::ForeFlightAhrs(ForeFlightAhrsMessage {
            roll_tenths_degrees: None,
            pitch_tenths_degrees: None,
            heading: None,
            indicated_airspeed_knots: None,
            true_airspeed_knots: None,
        })])
        .unwrap();
    }

    #[test]
    fn foreflight_message_set_rejects_unsupported_messages() {
        let error = validate_message_set(&[
            heartbeat(),
            Message::Initialization(crate::message::Initialization {
                audio_test: false,
                audio_inhibit: false,
                cdti_ok: true,
                csa_audio_disable: false,
                csa_disable: false,
            }),
        ])
        .unwrap_err();
        assert!(
            matches!(error, Gdl90Error::InvalidField { field, .. } if field == "ForeFlight supported message set")
        );
    }

    #[test]
    fn foreflight_datagram_enforces_whole_packet_mtu() {
        let oversized = vec![heartbeat(); 200];
        let error = encode_datagram(&oversized).unwrap_err();
        assert!(
            matches!(error, Gdl90Error::InvalidField { field, .. } if field == "ForeFlight datagram size")
        );
        assert_eq!(FOREFLIGHT_IPV4_UDP_PAYLOAD_LIMIT + 20 + 8, 1_499);
        assert_eq!(FOREFLIGHT_IPV6_UDP_PAYLOAD_LIMIT + 40 + 8, 1_499);
    }

    #[test]
    fn cadence_scheduler_coalesces_missed_intervals() {
        let mut scheduler = ForeFlightCadenceScheduler::new(Duration::ZERO);
        assert_eq!(
            scheduler.poll(Duration::ZERO),
            ForeFlightCadenceDue {
                ahrs: true,
                connectivity: true,
                discovery: true,
            }
        );
        assert_eq!(
            scheduler.poll(Duration::from_millis(199)),
            ForeFlightCadenceDue::default()
        );
        assert!(scheduler.poll(Duration::from_millis(200)).ahrs);
        let after_pause = scheduler.poll(Duration::from_secs(10));
        assert!(after_pause.ahrs);
        assert!(after_pause.connectivity);
        assert!(after_pause.discovery);
        assert_eq!(
            scheduler.poll(Duration::from_secs(10)),
            ForeFlightCadenceDue::default()
        );
    }
}
