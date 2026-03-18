use crate::error::{Gdl90Error, Result};
use crate::util::{decode_fixed_utf8, encode_fixed_utf8};

pub const FOREFLIGHT_MESSAGE_ID: u8 = 0x65;
pub const FOREFLIGHT_ID_SUB_ID: u8 = 0x00;
pub const FOREFLIGHT_AHRS_SUB_ID: u8 = 0x01;

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
    fn from_bits(bits: u8) -> Self {
        match bits {
            0 => Self::Unrestricted,
            1 => Self::Expensive,
            2 => Self::Disallowed,
            other => Self::Reserved(other),
        }
    }

    fn bits(self) -> u8 {
        match self {
            Self::Unrestricted => 0,
            Self::Expensive => 1,
            Self::Disallowed => 2,
            Self::Reserved(bits) => bits & 0x03,
        }
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
            internet_policy: InternetPolicy::from_bits(((raw >> 1) & 0x03) as u8),
            reserved_bits: raw & !0x07,
        }
    }

    pub fn raw(self) -> u32 {
        let datum = match self.geometric_altitude_datum {
            GeometricAltitudeDatum::Wgs84Ellipsoid => 0u32,
            GeometricAltitudeDatum::MeanSeaLevel => 1u32,
        };
        datum | ((self.internet_policy.bits() as u32) << 1) | (self.reserved_bits & !0x07)
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

    pub fn decode(payload: &[u8]) -> Result<Self> {
        if payload.len() != Self::LEN {
            return Err(Gdl90Error::InvalidLength {
                context: "ForeFlight ID message",
                expected: "39 bytes",
                actual: payload.len(),
            });
        }
        if payload[0] != FOREFLIGHT_MESSAGE_ID || payload[1] != FOREFLIGHT_ID_SUB_ID {
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

        Ok(Self {
            version,
            device_serial_number,
            device_name,
            device_long_name,
            capabilities,
        })
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        let mut out = Vec::with_capacity(Self::LEN);
        out.push(FOREFLIGHT_MESSAGE_ID);
        out.push(FOREFLIGHT_ID_SUB_ID);
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
        if payload[0] != FOREFLIGHT_MESSAGE_ID || payload[1] != FOREFLIGHT_AHRS_SUB_ID {
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
            if !(-3600..=3600).contains(&value) {
                return Err(Gdl90Error::InvalidField {
                    field: "AHRS heading",
                    details: format!("{value} is outside [-3600, 3600]"),
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
        out.push(FOREFLIGHT_AHRS_SUB_ID);
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
            if !(-3600..=3600).contains(&heading.tenths_degrees) {
                return Err(Gdl90Error::InvalidField {
                    field: "AHRS heading",
                    details: format!("{} is outside [-3600, 3600]", heading.tenths_degrees),
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
