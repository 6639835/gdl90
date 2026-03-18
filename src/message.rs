use crate::error::{Gdl90Error, Result};
use crate::foreflight::{
    FOREFLIGHT_AHRS_SUB_ID, FOREFLIGHT_ID_SUB_ID, FOREFLIGHT_MESSAGE_ID, ForeFlightAhrsMessage,
    ForeFlightIdMessage,
};
use crate::frame::{FrameDecoder, encode_frame};
use crate::uplink::UatUplinkPayload;
use crate::util::{
    decode_callsign, degrees_to_lat_lon, lat_lon_to_degrees, read_be_i16, read_be_i24, read_le_u24,
    write_le_u24,
};

pub const HEARTBEAT_MESSAGE_ID: u8 = 0;
pub const INITIALIZATION_MESSAGE_ID: u8 = 2;
pub const UPLINK_DATA_MESSAGE_ID: u8 = 7;
pub const HEIGHT_ABOVE_TERRAIN_MESSAGE_ID: u8 = 9;
pub const OWNSHIP_REPORT_MESSAGE_ID: u8 = 10;
pub const OWNSHIP_GEOMETRIC_ALTITUDE_MESSAGE_ID: u8 = 11;
pub const TRAFFIC_REPORT_MESSAGE_ID: u8 = 20;
pub const BASIC_REPORT_MESSAGE_ID: u8 = 30;
pub const LONG_REPORT_MESSAGE_ID: u8 = 31;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeartbeatStatus {
    pub gps_position_valid: bool,
    pub maintenance_required: bool,
    pub ident: bool,
    pub address_type_talkback: bool,
    pub gps_battery_low: bool,
    pub ratcs: bool,
    pub uat_initialized: bool,
    pub csa_requested: bool,
    pub csa_not_available: bool,
    pub utc_ok: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Heartbeat {
    pub status: HeartbeatStatus,
    pub timestamp_seconds_since_midnight: u32,
    pub uplink_count: u8,
    pub basic_and_long_count: u16,
}

impl Heartbeat {
    pub const LEN: usize = 7;

    pub fn decode(payload: &[u8]) -> Result<Self> {
        if payload.len() != Self::LEN {
            return Err(Gdl90Error::InvalidLength {
                context: "heartbeat message",
                expected: "7 bytes",
                actual: payload.len(),
            });
        }

        let status1 = payload[1];
        let status2 = payload[2];
        let timestamp =
            (((status2 >> 7) as u32) << 16) | ((payload[4] as u32) << 8) | payload[3] as u32;
        let uplink_count = payload[5] >> 3;
        let basic_and_long_count = (((payload[5] & 0x03) as u16) << 8) | payload[6] as u16;

        Ok(Self {
            status: HeartbeatStatus {
                gps_position_valid: (status1 & 0x80) != 0,
                maintenance_required: (status1 & 0x40) != 0,
                ident: (status1 & 0x20) != 0,
                address_type_talkback: (status1 & 0x10) != 0,
                gps_battery_low: (status1 & 0x08) != 0,
                ratcs: (status1 & 0x04) != 0,
                uat_initialized: (status1 & 0x01) != 0,
                csa_requested: (status2 & 0x40) != 0,
                csa_not_available: (status2 & 0x20) != 0,
                utc_ok: (status2 & 0x01) != 0,
            },
            timestamp_seconds_since_midnight: timestamp,
            uplink_count,
            basic_and_long_count,
        })
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        if self.timestamp_seconds_since_midnight > 0x1_FFFF {
            return Err(Gdl90Error::InvalidField {
                field: "heartbeat timestamp",
                details: "must fit in 17 bits".to_string(),
            });
        }
        if self.uplink_count > 0x1F {
            return Err(Gdl90Error::InvalidField {
                field: "heartbeat uplink count",
                details: "must fit in 5 bits".to_string(),
            });
        }
        if self.basic_and_long_count > 0x03FF {
            return Err(Gdl90Error::InvalidField {
                field: "heartbeat basic/long count",
                details: "must fit in 10 bits".to_string(),
            });
        }

        let mut out = Vec::with_capacity(Self::LEN);
        out.push(HEARTBEAT_MESSAGE_ID);
        let mut status1 = 0u8;
        status1 |= (self.status.gps_position_valid as u8) << 7;
        status1 |= (self.status.maintenance_required as u8) << 6;
        status1 |= (self.status.ident as u8) << 5;
        status1 |= (self.status.address_type_talkback as u8) << 4;
        status1 |= (self.status.gps_battery_low as u8) << 3;
        status1 |= (self.status.ratcs as u8) << 2;
        status1 |= self.status.uat_initialized as u8;
        out.push(status1);

        let mut status2 = 0u8;
        status2 |= (((self.timestamp_seconds_since_midnight >> 16) & 0x01) as u8) << 7;
        status2 |= (self.status.csa_requested as u8) << 6;
        status2 |= (self.status.csa_not_available as u8) << 5;
        status2 |= self.status.utc_ok as u8;
        out.push(status2);

        out.extend_from_slice(&(self.timestamp_seconds_since_midnight as u16).to_le_bytes());
        out.push((self.uplink_count << 3) | ((self.basic_and_long_count >> 8) as u8 & 0x03));
        out.push((self.basic_and_long_count & 0xFF) as u8);
        Ok(out)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Initialization {
    pub audio_test: bool,
    pub audio_inhibit: bool,
    pub cdti_ok: bool,
    pub csa_audio_disable: bool,
    pub csa_disable: bool,
}

impl Initialization {
    pub const LEN: usize = 3;

    pub fn decode(payload: &[u8]) -> Result<Self> {
        if payload.len() != Self::LEN {
            return Err(Gdl90Error::InvalidLength {
                context: "initialization message",
                expected: "3 bytes",
                actual: payload.len(),
            });
        }
        Ok(Self {
            audio_test: (payload[1] & 0x40) != 0,
            audio_inhibit: (payload[1] & 0x02) != 0,
            cdti_ok: (payload[1] & 0x01) != 0,
            csa_audio_disable: (payload[2] & 0x02) != 0,
            csa_disable: (payload[2] & 0x01) != 0,
        })
    }

    pub fn encode(&self) -> Vec<u8> {
        vec![
            INITIALIZATION_MESSAGE_ID,
            ((self.audio_test as u8) << 6) | ((self.audio_inhibit as u8) << 1) | self.cdti_ok as u8,
            ((self.csa_audio_disable as u8) << 1) | self.csa_disable as u8,
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UplinkData {
    pub time_of_reception: Option<u32>,
    pub payload: UatUplinkPayload,
}

impl UplinkData {
    pub const LEN: usize = 436;

    pub fn decode(payload: &[u8]) -> Result<Self> {
        if payload.len() != Self::LEN {
            return Err(Gdl90Error::InvalidLength {
                context: "uplink data message",
                expected: "436 bytes",
                actual: payload.len(),
            });
        }
        let tor = read_le_u24(&payload[1..4]);
        let payload = UatUplinkPayload::from_bytes(&payload[4..])?;
        Ok(Self {
            time_of_reception: if tor == 0xFF_FFFF { None } else { Some(tor) },
            payload,
        })
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        let mut out = Vec::with_capacity(Self::LEN);
        out.push(UPLINK_DATA_MESSAGE_ID);
        out.extend_from_slice(&write_le_u24(self.time_of_reception.unwrap_or(0xFF_FFFF))?);
        out.extend_from_slice(&self.payload.as_bytes());
        Ok(out)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetAlertStatus {
    NoAlert,
    TrafficAlert,
    Reserved(u8),
}

impl TargetAlertStatus {
    fn from_raw(value: u8) -> Self {
        match value {
            0 => Self::NoAlert,
            1 => Self::TrafficAlert,
            other => Self::Reserved(other),
        }
    }

    fn raw(self) -> u8 {
        match self {
            Self::NoAlert => 0,
            Self::TrafficAlert => 1,
            Self::Reserved(value) => value & 0x0F,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressType {
    AdsbIcao,
    AdsbSelfAssigned,
    TisbIcao,
    TisbTrackFile,
    SurfaceVehicle,
    GroundStationBeacon,
    Reserved(u8),
}

impl AddressType {
    fn from_raw(value: u8) -> Self {
        match value {
            0 => Self::AdsbIcao,
            1 => Self::AdsbSelfAssigned,
            2 => Self::TisbIcao,
            3 => Self::TisbTrackFile,
            4 => Self::SurfaceVehicle,
            5 => Self::GroundStationBeacon,
            other => Self::Reserved(other),
        }
    }

    fn raw(self) -> u8 {
        match self {
            Self::AdsbIcao => 0,
            Self::AdsbSelfAssigned => 1,
            Self::TisbIcao => 2,
            Self::TisbTrackFile => 3,
            Self::SurfaceVehicle => 4,
            Self::GroundStationBeacon => 5,
            Self::Reserved(value) => value & 0x0F,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackType {
    NotValid,
    TrueTrack,
    MagneticHeading,
    TrueHeading,
}

impl TrackType {
    fn from_raw(value: u8) -> Self {
        match value & 0x03 {
            0 => Self::NotValid,
            1 => Self::TrueTrack,
            2 => Self::MagneticHeading,
            _ => Self::TrueHeading,
        }
    }

    fn raw(self) -> u8 {
        match self {
            Self::NotValid => 0,
            Self::TrueTrack => 1,
            Self::MagneticHeading => 2,
            Self::TrueHeading => 3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetMisc {
    pub airborne: bool,
    pub extrapolated: bool,
    pub track_type: TrackType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerticalFigureOfMerit {
    Meters(u16),
    NotAvailable,
    GreaterThan32766,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TargetReport {
    pub alert_status: TargetAlertStatus,
    pub address_type: AddressType,
    pub participant_address: u32,
    pub latitude_degrees: f64,
    pub longitude_degrees: f64,
    pub pressure_altitude_feet: Option<i32>,
    pub misc: TargetMisc,
    pub nic: u8,
    pub nacp: u8,
    pub horizontal_velocity_knots: Option<u16>,
    pub vertical_velocity_fpm: Option<i16>,
    pub track_heading: Option<u8>,
    pub emitter_category: u8,
    pub call_sign: String,
    pub emergency_priority_code: u8,
    pub spare: u8,
}

impl TargetReport {
    pub const LEN: usize = 28;

    pub fn decode(payload: &[u8]) -> Result<Self> {
        if payload.len() != Self::LEN {
            return Err(Gdl90Error::InvalidLength {
                context: "target report",
                expected: "28 bytes",
                actual: payload.len(),
            });
        }

        let alert_status = TargetAlertStatus::from_raw(payload[1] >> 4);
        let address_type = AddressType::from_raw(payload[1] & 0x0F);
        let participant_address =
            ((payload[2] as u32) << 16) | ((payload[3] as u32) << 8) | payload[4] as u32;

        let latitude_raw = read_be_i24(&payload[5..8]);
        let longitude_raw = read_be_i24(&payload[8..11]);
        let altitude_raw = ((payload[11] as u16) << 4) | ((payload[12] as u16) >> 4);
        let pressure_altitude_feet = if altitude_raw == 0x0FFF {
            None
        } else {
            Some(i32::from(altitude_raw) * 25 - 1000)
        };

        let misc_raw = payload[12] & 0x0F;
        let nic = payload[13] >> 4;
        let nacp = payload[13] & 0x0F;

        let horizontal_raw = ((payload[14] as u16) << 4) | ((payload[15] as u16) >> 4);
        let horizontal_velocity_knots = if horizontal_raw == 0x0FFF {
            None
        } else {
            Some(horizontal_raw.min(0x0FFE))
        };

        let vertical_raw = (((payload[15] & 0x0F) as u16) << 8) | payload[16] as u16;
        let vertical_velocity_fpm = decode_vertical_velocity(vertical_raw);

        let track_type = TrackType::from_raw(misc_raw);
        let track_heading = if matches!(track_type, TrackType::NotValid) {
            None
        } else {
            Some(payload[17])
        };

        let mut call_sign_bytes = [0u8; 8];
        call_sign_bytes.copy_from_slice(&payload[19..27]);

        Ok(Self {
            alert_status,
            address_type,
            participant_address,
            latitude_degrees: lat_lon_to_degrees(latitude_raw),
            longitude_degrees: lat_lon_to_degrees(longitude_raw),
            pressure_altitude_feet,
            misc: TargetMisc {
                airborne: (misc_raw & 0x08) != 0,
                extrapolated: (misc_raw & 0x04) != 0,
                track_type,
            },
            nic,
            nacp,
            horizontal_velocity_knots,
            vertical_velocity_fpm,
            track_heading,
            emitter_category: payload[18],
            call_sign: decode_callsign(&call_sign_bytes),
            emergency_priority_code: payload[27] >> 4,
            spare: payload[27] & 0x0F,
        })
    }

    pub fn encode(&self, message_id: u8) -> Result<Vec<u8>> {
        if self.participant_address > 0xFF_FFFF {
            return Err(Gdl90Error::InvalidField {
                field: "participant address",
                details: "must fit in 24 bits".to_string(),
            });
        }
        if self.nic > 0x0F || self.nacp > 0x0F {
            return Err(Gdl90Error::InvalidField {
                field: "NIC/NACp",
                details: "must fit in 4 bits".to_string(),
            });
        }
        if self.emergency_priority_code > 0x0F || self.spare > 0x0F {
            return Err(Gdl90Error::InvalidField {
                field: "emergency/spare nibble",
                details: "must fit in 4 bits".to_string(),
            });
        }
        if self.emitter_category > 39 {
            return Err(Gdl90Error::InvalidField {
                field: "emitter category",
                details: "must be in the range 0..=39".to_string(),
            });
        }

        let mut out = Vec::with_capacity(Self::LEN);
        out.push(message_id);
        out.push((self.alert_status.raw() << 4) | self.address_type.raw());
        out.push(((self.participant_address >> 16) & 0xFF) as u8);
        out.push(((self.participant_address >> 8) & 0xFF) as u8);
        out.push((self.participant_address & 0xFF) as u8);
        out.extend_from_slice(&degrees_to_lat_lon(
            self.latitude_degrees,
            "latitude",
            -90.0,
            90.0,
        )?);
        out.extend_from_slice(&degrees_to_lat_lon(
            self.longitude_degrees,
            "longitude",
            -180.0,
            179.999_978_542_327_88,
        )?);

        let altitude_raw = if let Some(feet) = self.pressure_altitude_feet {
            if feet < -1000 {
                return Err(Gdl90Error::InvalidField {
                    field: "pressure altitude",
                    details: "must be >= -1000 feet".to_string(),
                });
            }
            let adjusted = feet + 1000;
            if adjusted % 25 != 0 {
                return Err(Gdl90Error::InvalidField {
                    field: "pressure altitude",
                    details: "must be a 25-foot increment".to_string(),
                });
            }
            let encoded = adjusted / 25;
            if encoded > 0x0FFE {
                return Err(Gdl90Error::InvalidField {
                    field: "pressure altitude",
                    details: "exceeds maximum encodable altitude".to_string(),
                });
            }
            encoded as u16
        } else {
            0x0FFF
        };

        let misc = ((self.misc.airborne as u8) << 3)
            | ((self.misc.extrapolated as u8) << 2)
            | self.misc.track_type.raw();
        out.push((altitude_raw >> 4) as u8);
        out.push(((altitude_raw as u8 & 0x0F) << 4) | misc);
        out.push((self.nic << 4) | self.nacp);

        let horizontal = match self.horizontal_velocity_knots {
            Some(knots) => knots.min(0x0FFE),
            None => 0x0FFF,
        };
        if horizontal > 0x0FFF {
            return Err(Gdl90Error::InvalidField {
                field: "horizontal velocity",
                details: "must fit in 12 bits".to_string(),
            });
        }
        let vertical = encode_vertical_velocity(self.vertical_velocity_fpm)?;
        out.push((horizontal >> 4) as u8);
        out.push(((horizontal as u8 & 0x0F) << 4) | ((vertical >> 8) as u8 & 0x0F));
        out.push((vertical & 0xFF) as u8);
        out.push(match self.misc.track_type {
            TrackType::NotValid => 0,
            _ => self.track_heading.unwrap_or(0),
        });
        out.push(self.emitter_category);

        let mut call_sign = [b' '; 8];
        let encoded = self.call_sign.to_ascii_uppercase();
        if encoded.len() > 8 {
            return Err(Gdl90Error::InvalidField {
                field: "call sign",
                details: "must be at most 8 characters".to_string(),
            });
        }
        for (index, byte) in encoded.bytes().enumerate() {
            if !matches!(byte, b'0'..=b'9' | b'A'..=b'Z' | b' ' | b'-') {
                return Err(Gdl90Error::InvalidField {
                    field: "call sign",
                    details: format!("byte {byte:#04x} is not permitted"),
                });
            }
            call_sign[index] = byte;
        }
        out.extend_from_slice(&call_sign);
        out.push((self.emergency_priority_code << 4) | self.spare);
        Ok(out)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PassThroughReport<const N: usize> {
    pub time_of_reception: Option<u32>,
    pub payload: [u8; N],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UatAdsbPayloadHeader {
    pub payload_type_code: u8,
    pub address_qualifier: u8,
    pub address: u32,
}

impl UatAdsbPayloadHeader {
    pub const LEN: usize = 4;

    pub fn decode(payload: &[u8]) -> Result<Self> {
        if payload.len() != Self::LEN {
            return Err(Gdl90Error::InvalidLength {
                context: "UAT ADS-B payload header",
                expected: "4 bytes",
                actual: payload.len(),
            });
        }

        Ok(Self {
            payload_type_code: payload[0] >> 3,
            address_qualifier: payload[0] & 0x07,
            address: ((payload[1] as u32) << 16) | ((payload[2] as u32) << 8) | payload[3] as u32,
        })
    }

    pub fn encode(&self) -> Result<[u8; Self::LEN]> {
        if self.payload_type_code > 0x1F {
            return Err(Gdl90Error::InvalidField {
                field: "UAT ADS-B payload type code",
                details: "must fit in 5 bits".to_string(),
            });
        }
        if self.address_qualifier > 0x07 {
            return Err(Gdl90Error::InvalidField {
                field: "UAT ADS-B address qualifier",
                details: "must fit in 3 bits".to_string(),
            });
        }
        if self.address > 0xFF_FFFF {
            return Err(Gdl90Error::InvalidField {
                field: "UAT ADS-B address",
                details: "must fit in 24 bits".to_string(),
            });
        }

        Ok([
            (self.payload_type_code << 3) | self.address_qualifier,
            ((self.address >> 16) & 0xFF) as u8,
            ((self.address >> 8) & 0xFF) as u8,
            (self.address & 0xFF) as u8,
        ])
    }

    pub fn is_basic(self) -> bool {
        self.payload_type_code == 0
    }

    pub fn is_long_type1(self) -> bool {
        self.payload_type_code == 1
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BasicUatPayload {
    pub header: UatAdsbPayloadHeader,
    pub state_vector: [u8; 13],
    pub reserved: u8,
}

impl BasicUatPayload {
    pub const LEN: usize = 18;

    pub fn decode(payload: &[u8]) -> Result<Self> {
        if payload.len() != Self::LEN {
            return Err(Gdl90Error::InvalidLength {
                context: "Basic UAT payload",
                expected: "18 bytes",
                actual: payload.len(),
            });
        }

        let header = UatAdsbPayloadHeader::decode(&payload[..4])?;
        let mut state_vector = [0u8; 13];
        state_vector.copy_from_slice(&payload[4..17]);

        Ok(Self {
            header,
            state_vector,
            reserved: payload[17],
        })
    }

    pub fn encode(&self) -> Result<[u8; Self::LEN]> {
        let mut out = [0u8; Self::LEN];
        out[..4].copy_from_slice(&self.header.encode()?);
        out[4..17].copy_from_slice(&self.state_vector);
        out[17] = self.reserved;
        Ok(out)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LongUatPayload {
    pub header: UatAdsbPayloadHeader,
    pub state_vector: [u8; 13],
    pub mode_status: [u8; 12],
    pub auxiliary_state_vector: [u8; 5],
}

impl LongUatPayload {
    pub const LEN: usize = 34;

    pub fn decode(payload: &[u8]) -> Result<Self> {
        if payload.len() != Self::LEN {
            return Err(Gdl90Error::InvalidLength {
                context: "Long UAT payload",
                expected: "34 bytes",
                actual: payload.len(),
            });
        }

        let header = UatAdsbPayloadHeader::decode(&payload[..4])?;
        let mut state_vector = [0u8; 13];
        state_vector.copy_from_slice(&payload[4..17]);
        let mut mode_status = [0u8; 12];
        mode_status.copy_from_slice(&payload[17..29]);
        let mut auxiliary_state_vector = [0u8; 5];
        auxiliary_state_vector.copy_from_slice(&payload[29..34]);

        Ok(Self {
            header,
            state_vector,
            mode_status,
            auxiliary_state_vector,
        })
    }

    pub fn encode(&self) -> Result<[u8; Self::LEN]> {
        let mut out = [0u8; Self::LEN];
        out[..4].copy_from_slice(&self.header.encode()?);
        out[4..17].copy_from_slice(&self.state_vector);
        out[17..29].copy_from_slice(&self.mode_status);
        out[29..34].copy_from_slice(&self.auxiliary_state_vector);
        Ok(out)
    }
}

impl<const N: usize> PassThroughReport<N> {
    pub fn decode(message_name: &'static str, payload: &[u8]) -> Result<Self> {
        if payload.len() != N + 4 {
            return Err(Gdl90Error::InvalidLength {
                context: message_name,
                expected: "message specific fixed length",
                actual: payload.len(),
            });
        }
        let tor = read_le_u24(&payload[1..4]);
        let mut data = [0u8; N];
        data.copy_from_slice(&payload[4..]);
        Ok(Self {
            time_of_reception: if tor == 0xFF_FFFF { None } else { Some(tor) },
            payload: data,
        })
    }

    pub fn encode(&self, message_id: u8) -> Result<Vec<u8>> {
        let mut out = Vec::with_capacity(N + 4);
        out.push(message_id);
        out.extend_from_slice(&crate::util::write_le_u24(
            self.time_of_reception.unwrap_or(0xFF_FFFF),
        )?);
        out.extend_from_slice(&self.payload);
        Ok(out)
    }
}

impl PassThroughReport<18> {
    pub fn basic_payload(&self) -> BasicUatPayload {
        BasicUatPayload::decode(&self.payload).expect("fixed-size basic payload should decode")
    }

    pub fn from_basic_payload(
        time_of_reception: Option<u32>,
        payload: &BasicUatPayload,
    ) -> Result<Self> {
        Ok(Self {
            time_of_reception,
            payload: payload.encode()?,
        })
    }
}

impl PassThroughReport<34> {
    pub fn long_payload(&self) -> LongUatPayload {
        LongUatPayload::decode(&self.payload).expect("fixed-size long payload should decode")
    }

    pub fn from_long_payload(
        time_of_reception: Option<u32>,
        payload: &LongUatPayload,
    ) -> Result<Self> {
        Ok(Self {
            time_of_reception,
            payload: payload.encode()?,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeightAboveTerrain {
    pub feet: Option<i16>,
}

impl HeightAboveTerrain {
    pub const LEN: usize = 3;

    pub fn decode(payload: &[u8]) -> Result<Self> {
        if payload.len() != Self::LEN {
            return Err(Gdl90Error::InvalidLength {
                context: "height above terrain message",
                expected: "3 bytes",
                actual: payload.len(),
            });
        }
        let raw = read_be_i16(&payload[1..3]);
        Ok(Self {
            feet: if raw == i16::MIN { None } else { Some(raw) },
        })
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(Self::LEN);
        out.push(HEIGHT_ABOVE_TERRAIN_MESSAGE_ID);
        out.extend_from_slice(&self.feet.unwrap_or(i16::MIN).to_be_bytes());
        out
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OwnshipGeometricAltitude {
    pub altitude_feet: i32,
    pub vertical_warning: bool,
    pub vertical_figure_of_merit: VerticalFigureOfMerit,
}

impl OwnshipGeometricAltitude {
    pub const LEN: usize = 5;

    pub fn decode(payload: &[u8]) -> Result<Self> {
        if payload.len() != Self::LEN {
            return Err(Gdl90Error::InvalidLength {
                context: "ownship geometric altitude message",
                expected: "5 bytes",
                actual: payload.len(),
            });
        }
        let raw_altitude = i16::from_be_bytes([payload[1], payload[2]]);
        let raw_metrics = u16::from_be_bytes([payload[3], payload[4]]);
        Ok(Self {
            altitude_feet: i32::from(raw_altitude) * 5,
            vertical_warning: (raw_metrics & 0x8000) != 0,
            vertical_figure_of_merit: match raw_metrics & 0x7FFF {
                0x7FFF => VerticalFigureOfMerit::NotAvailable,
                0x7FFE => VerticalFigureOfMerit::GreaterThan32766,
                meters => VerticalFigureOfMerit::Meters(meters),
            },
        })
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        if self.altitude_feet % 5 != 0 {
            return Err(Gdl90Error::InvalidField {
                field: "geometric altitude",
                details: "must be a 5-foot increment".to_string(),
            });
        }
        let altitude_units = self.altitude_feet / 5;
        if !(i32::from(i16::MIN)..=i32::from(i16::MAX)).contains(&altitude_units) {
            return Err(Gdl90Error::InvalidField {
                field: "geometric altitude",
                details: "does not fit in signed 16-bit 5-foot units".to_string(),
            });
        }
        let vfom = match self.vertical_figure_of_merit {
            VerticalFigureOfMerit::Meters(value) => value,
            VerticalFigureOfMerit::NotAvailable => 0x7FFF,
            VerticalFigureOfMerit::GreaterThan32766 => 0x7FFE,
        };

        let mut out = Vec::with_capacity(Self::LEN);
        out.push(OWNSHIP_GEOMETRIC_ALTITUDE_MESSAGE_ID);
        out.extend_from_slice(&(altitude_units as i16).to_be_bytes());
        out.extend_from_slice(
            &(((self.vertical_warning as u16) << 15) | (vfom & 0x7FFF)).to_be_bytes(),
        );
        Ok(out)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    Heartbeat(Heartbeat),
    Initialization(Initialization),
    UplinkData(UplinkData),
    HeightAboveTerrain(HeightAboveTerrain),
    OwnshipReport(TargetReport),
    OwnshipGeometricAltitude(OwnshipGeometricAltitude),
    TrafficReport(TargetReport),
    BasicReport(PassThroughReport<18>),
    LongReport(PassThroughReport<34>),
    ForeFlightId(ForeFlightIdMessage),
    ForeFlightAhrs(ForeFlightAhrsMessage),
    Unknown { message_id: u8, data: Vec<u8> },
}

impl Message {
    pub fn kind_name(&self) -> String {
        match self {
            Self::Heartbeat(_) => "Heartbeat".to_string(),
            Self::Initialization(_) => "Initialization".to_string(),
            Self::UplinkData(_) => "UplinkData".to_string(),
            Self::HeightAboveTerrain(_) => "HeightAboveTerrain".to_string(),
            Self::OwnshipReport(_) => "OwnshipReport".to_string(),
            Self::OwnshipGeometricAltitude(_) => "OwnshipGeometricAltitude".to_string(),
            Self::TrafficReport(_) => "TrafficReport".to_string(),
            Self::BasicReport(_) => "BasicReport".to_string(),
            Self::LongReport(_) => "LongReport".to_string(),
            Self::ForeFlightId(_) => "ForeFlightId".to_string(),
            Self::ForeFlightAhrs(_) => "ForeFlightAhrs".to_string(),
            Self::Unknown { message_id, .. } => format!("Unknown({message_id:#04x})"),
        }
    }

    pub fn summary(&self) -> String {
        match self {
            Self::Heartbeat(message) => format!(
                "utc={} gps_valid={} uplinks={} basic_long={}",
                message.timestamp_seconds_since_midnight,
                message.status.gps_position_valid,
                message.uplink_count,
                message.basic_and_long_count
            ),
            Self::Initialization(message) => format!(
                "audio_test={} audio_inhibit={} cdti_ok={} csa_disable={}",
                message.audio_test, message.audio_inhibit, message.cdti_ok, message.csa_disable
            ),
            Self::UplinkData(message) => format!(
                "tor={:?} application_data={} bytes",
                message.time_of_reception,
                message.payload.application_data.len()
            ),
            Self::HeightAboveTerrain(message) => format!("feet={:?}", message.feet),
            Self::OwnshipReport(message) => format_target_summary("ownship", message),
            Self::OwnshipGeometricAltitude(message) => format!(
                "altitude_ft={} vertical_warning={}",
                message.altitude_feet, message.vertical_warning
            ),
            Self::TrafficReport(message) => format_target_summary("traffic", message),
            Self::BasicReport(message) => {
                let payload = message.basic_payload();
                format!(
                    "tor={:?} type={} qualifier={} address={:#08x}",
                    message.time_of_reception,
                    payload.header.payload_type_code,
                    payload.header.address_qualifier,
                    payload.header.address
                )
            }
            Self::LongReport(message) => {
                let payload = message.long_payload();
                format!(
                    "tor={:?} type={} qualifier={} address={:#08x}",
                    message.time_of_reception,
                    payload.header.payload_type_code,
                    payload.header.address_qualifier,
                    payload.header.address
                )
            }
            Self::ForeFlightId(message) => format!(
                "version={} name={} long_name={}",
                message.version, message.device_name, message.device_long_name
            ),
            Self::ForeFlightAhrs(message) => format!(
                "roll={:?} pitch={:?} heading={:?}",
                message.roll_tenths_degrees,
                message.pitch_tenths_degrees,
                message.heading.map(|heading| heading.tenths_degrees)
            ),
            Self::Unknown { message_id, data } => {
                format!("message_id={message_id:#04x} payload={} bytes", data.len())
            }
        }
    }

    pub fn message_id(&self) -> u8 {
        match self {
            Self::Heartbeat(_) => HEARTBEAT_MESSAGE_ID,
            Self::Initialization(_) => INITIALIZATION_MESSAGE_ID,
            Self::UplinkData(_) => UPLINK_DATA_MESSAGE_ID,
            Self::HeightAboveTerrain(_) => HEIGHT_ABOVE_TERRAIN_MESSAGE_ID,
            Self::OwnshipReport(_) => OWNSHIP_REPORT_MESSAGE_ID,
            Self::OwnshipGeometricAltitude(_) => OWNSHIP_GEOMETRIC_ALTITUDE_MESSAGE_ID,
            Self::TrafficReport(_) => TRAFFIC_REPORT_MESSAGE_ID,
            Self::BasicReport(_) => BASIC_REPORT_MESSAGE_ID,
            Self::LongReport(_) => LONG_REPORT_MESSAGE_ID,
            Self::ForeFlightId(_) | Self::ForeFlightAhrs(_) => FOREFLIGHT_MESSAGE_ID,
            Self::Unknown { message_id, .. } => *message_id,
        }
    }

    pub fn decode(payload: &[u8]) -> Result<Self> {
        if payload.is_empty() {
            return Err(Gdl90Error::FrameTooShort);
        }
        if payload[0] > 127 {
            return Err(Gdl90Error::InvalidMessageId(payload[0]));
        }

        match payload[0] {
            HEARTBEAT_MESSAGE_ID => Ok(Self::Heartbeat(Heartbeat::decode(payload)?)),
            INITIALIZATION_MESSAGE_ID => Ok(Self::Initialization(Initialization::decode(payload)?)),
            UPLINK_DATA_MESSAGE_ID => Ok(Self::UplinkData(UplinkData::decode(payload)?)),
            HEIGHT_ABOVE_TERRAIN_MESSAGE_ID => Ok(Self::HeightAboveTerrain(
                HeightAboveTerrain::decode(payload)?,
            )),
            OWNSHIP_REPORT_MESSAGE_ID => Ok(Self::OwnshipReport(TargetReport::decode(payload)?)),
            OWNSHIP_GEOMETRIC_ALTITUDE_MESSAGE_ID => Ok(Self::OwnshipGeometricAltitude(
                OwnshipGeometricAltitude::decode(payload)?,
            )),
            TRAFFIC_REPORT_MESSAGE_ID => Ok(Self::TrafficReport(TargetReport::decode(payload)?)),
            BASIC_REPORT_MESSAGE_ID => Ok(Self::BasicReport(PassThroughReport::<18>::decode(
                "basic report",
                payload,
            )?)),
            LONG_REPORT_MESSAGE_ID => Ok(Self::LongReport(PassThroughReport::<34>::decode(
                "long report",
                payload,
            )?)),
            FOREFLIGHT_MESSAGE_ID => match payload.get(1).copied() {
                Some(FOREFLIGHT_ID_SUB_ID) => {
                    Ok(Self::ForeFlightId(ForeFlightIdMessage::decode(payload)?))
                }
                Some(FOREFLIGHT_AHRS_SUB_ID) => Ok(Self::ForeFlightAhrs(
                    ForeFlightAhrsMessage::decode(payload)?,
                )),
                _ => Ok(Self::Unknown {
                    message_id: payload[0],
                    data: payload[1..].to_vec(),
                }),
            },
            other => Ok(Self::Unknown {
                message_id: other,
                data: payload[1..].to_vec(),
            }),
        }
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        match self {
            Self::Heartbeat(message) => message.encode(),
            Self::Initialization(message) => Ok(message.encode()),
            Self::UplinkData(message) => message.encode(),
            Self::HeightAboveTerrain(message) => Ok(message.encode()),
            Self::OwnshipReport(message) => message.encode(OWNSHIP_REPORT_MESSAGE_ID),
            Self::OwnshipGeometricAltitude(message) => message.encode(),
            Self::TrafficReport(message) => message.encode(TRAFFIC_REPORT_MESSAGE_ID),
            Self::BasicReport(message) => message.encode(BASIC_REPORT_MESSAGE_ID),
            Self::LongReport(message) => message.encode(LONG_REPORT_MESSAGE_ID),
            Self::ForeFlightId(message) => message.encode(),
            Self::ForeFlightAhrs(message) => message.encode(),
            Self::Unknown { message_id, data } => {
                let mut out = Vec::with_capacity(1 + data.len());
                out.push(*message_id);
                out.extend_from_slice(data);
                Ok(out)
            }
        }
    }

    pub fn encode_frame(&self) -> Result<Vec<u8>> {
        Ok(encode_frame(&self.encode()?))
    }
}

fn format_target_summary(label: &str, message: &TargetReport) -> String {
    format!(
        "{label} addr={:#08x} callsign={} lat={:.5} lon={:.5} alt_ft={:?}",
        message.participant_address,
        message.call_sign,
        message.latitude_degrees,
        message.longitude_degrees,
        message.pressure_altitude_feet
    )
}

#[derive(Debug, Default, Clone)]
pub struct FrameMessageDecoder {
    frame_decoder: FrameDecoder,
}

impl FrameMessageDecoder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, bytes: &[u8]) -> Vec<Result<Message>> {
        self.frame_decoder
            .push(bytes)
            .into_iter()
            .map(|result| result.and_then(|payload| Message::decode(&payload)))
            .collect()
    }

    pub fn reset(&mut self) {
        self.frame_decoder.reset();
    }
}

fn decode_vertical_velocity(raw: u16) -> Option<i16> {
    match raw {
        0x0800 => None,
        0x0000..=0x01FE => Some((raw as i16) * 64),
        0x0E02..=0x0FFF => {
            let signed = ((raw as i16) << 4) >> 4;
            Some(signed * 64)
        }
        _ => None,
    }
}

fn encode_vertical_velocity(value: Option<i16>) -> Result<u16> {
    let value = if let Some(value) = value {
        if value % 64 != 0 {
            return Err(Gdl90Error::InvalidField {
                field: "vertical velocity",
                details: "must be a 64 fpm increment".to_string(),
            });
        }
        let units = value / 64;
        if units >= 510 {
            0x01FE
        } else if units <= -510 {
            0x0E02
        } else {
            (units as i16 as u16) & 0x0FFF
        }
    } else {
        0x0800
    };
    Ok(value)
}
