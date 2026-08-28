from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def write(path: str, content: str) -> None:
    (ROOT / path).write_text(content.rstrip() + "\n", encoding="utf-8")


def replace_once(path: str, old: str, new: str) -> None:
    content = read(path)
    count = content.count(old)
    if count != 1:
        raise RuntimeError(f"{path}: expected one exact match, found {count}")
    write(path, content.replace(old, new, 1))


def insert_before_last_brace(path: str, addition: str) -> None:
    content = read(path).rstrip()
    index = content.rfind("\n}")
    if index < 0:
        raise RuntimeError(f"{path}: final module brace not found")
    write(path, content[:index] + "\n" + addition.rstrip() + content[index:])


# ForeFlight sentinel, heading-range, and fixed-field edge cases.
replace_once(
    "src/foreflight.rs",
    '''        if self.version != 1 {
            return Err(Gdl90Error::InvalidField {
                field: "ForeFlight ID version",
                details: format!("{} is not the documented version 1", self.version),
            });
        }
        self.capabilities.validate()''',
    '''        if self.version != 1 {
            return Err(Gdl90Error::InvalidField {
                field: "ForeFlight ID version",
                details: format!("{} is not the documented version 1", self.version),
            });
        }
        if self.device_serial_number == Some(u64::MAX) {
            return Err(Gdl90Error::InvalidField {
                field: "ForeFlight ID serial number",
                details: "0xFFFFFFFFFFFFFFFF is reserved for an unavailable serial number"
                    .to_string(),
            });
        }
        self.capabilities.validate()''',
)

replace_once(
    "src/foreflight.rs",
    '''        let heading = if let Some(heading) = self.heading {
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
        };''',
    '''        let heading = if let Some(heading) = self.heading {
            let normalized = normalize_heading_tenths_degrees(heading.tenths_degrees)?;
            let type_bit = match heading.heading_type {
                HeadingType::True => 0u16,
                HeadingType::Magnetic => 0x8000,
            };
            type_bit | normalized
        } else {
            0xFFFF
        };''',
)

replace_once(
    "src/foreflight.rs",
    '''        out.extend_from_slice(&heading.to_be_bytes());
        out.extend_from_slice(
            &self
                .indicated_airspeed_knots
                .unwrap_or(0xFFFF)
                .to_be_bytes(),
        );
        out.extend_from_slice(&self.true_airspeed_knots.unwrap_or(0xFFFF).to_be_bytes());
        Ok(out)''',
    '''        out.extend_from_slice(&heading.to_be_bytes());
        out.extend_from_slice(&encode_optional_u16(
            self.indicated_airspeed_knots,
            "AHRS indicated airspeed",
        )?);
        out.extend_from_slice(&encode_optional_u16(
            self.true_airspeed_knots,
            "AHRS true airspeed",
        )?);
        Ok(out)''',
)

replace_once(
    "src/foreflight.rs",
    '''fn decode_optional_u16(value: u16) -> Option<u16> {
    if value == 0xFFFF { None } else { Some(value) }
}

fn decode_optional_signed_range(''',
    '''fn decode_optional_u16(value: u16) -> Option<u16> {
    if value == 0xFFFF { None } else { Some(value) }
}

fn encode_optional_u16(value: Option<u16>, field: &'static str) -> Result<[u8; 2]> {
    if value == Some(u16::MAX) {
        return Err(Gdl90Error::InvalidField {
            field,
            details: "0xFFFF is reserved for an unavailable value".to_string(),
        });
    }
    Ok(value.unwrap_or(u16::MAX).to_be_bytes())
}

/// ForeFlight publishes an accepted range of -360.0 through +360.0 degrees,
/// but allocates all 15 value bits to an unsigned heading. Negative API inputs
/// are therefore canonicalized to their equivalent positive angular heading.
fn normalize_heading_tenths_degrees(value: i16) -> Result<u16> {
    if !(-3600..=3600).contains(&value) {
        return Err(Gdl90Error::InvalidField {
            field: "AHRS heading",
            details: format!("{value} is outside [-3600, 3600]"),
        });
    }
    Ok(if value < 0 { value + 3600 } else { value } as u16)
}

fn decode_optional_signed_range(''',
)

insert_before_last_brace(
    "src/foreflight.rs",
    r'''
    #[test]
    fn negative_headings_are_accepted_and_canonicalized() {
        let message = ForeFlightAhrsMessage {
            roll_tenths_degrees: None,
            pitch_tenths_degrees: None,
            heading: Some(Heading {
                heading_type: HeadingType::Magnetic,
                tenths_degrees: -10,
            }),
            indicated_airspeed_knots: None,
            true_airspeed_knots: None,
        };
        let encoded = message.encode().unwrap();
        assert_eq!(u16::from_be_bytes([encoded[6], encoded[7]]) & 0x7FFF, 3590);
        assert_eq!(
            ForeFlightAhrsMessage::decode(&encoded)
                .unwrap()
                .heading
                .unwrap()
                .tenths_degrees,
            3590
        );

        let mut invalid = message;
        invalid.heading.as_mut().unwrap().tenths_degrees = -3601;
        assert!(matches!(
            invalid.encode(),
            Err(Gdl90Error::InvalidField {
                field: "AHRS heading",
                ..
            })
        ));
    }

    #[test]
    fn foreflight_invalid_sentinels_cannot_be_encoded_as_real_values() {
        let id = ForeFlightIdMessage {
            version: 1,
            device_serial_number: Some(u64::MAX),
            device_name: "GDL90".to_string(),
            device_long_name: "GDL90".to_string(),
            capabilities: ForeFlightCapabilities {
                geometric_altitude_datum: GeometricAltitudeDatum::Wgs84Ellipsoid,
                internet_policy: InternetPolicy::Unrestricted,
                reserved_bits: 0,
            },
        };
        assert!(matches!(
            id.encode(),
            Err(Gdl90Error::InvalidField {
                field: "ForeFlight ID serial number",
                ..
            })
        ));

        let ahrs = ForeFlightAhrsMessage {
            roll_tenths_degrees: None,
            pitch_tenths_degrees: None,
            heading: None,
            indicated_airspeed_knots: Some(u16::MAX),
            true_airspeed_knots: None,
        };
        assert!(matches!(
            ahrs.encode(),
            Err(Gdl90Error::InvalidField {
                field: "AHRS indicated airspeed",
                ..
            })
        ));
    }

    #[test]
    fn foreflight_names_reject_embedded_nul_and_nonzero_trailing_padding() {
        let message = ForeFlightIdMessage {
            version: 1,
            device_serial_number: None,
            device_name: "A\0B".to_string(),
            device_long_name: "GDL90".to_string(),
            capabilities: ForeFlightCapabilities {
                geometric_altitude_datum: GeometricAltitudeDatum::Wgs84Ellipsoid,
                internet_policy: InternetPolicy::Unrestricted,
                reserved_bits: 0,
            },
        };
        assert!(matches!(
            message.encode(),
            Err(Gdl90Error::InvalidField {
                field: "device name",
                ..
            })
        ));

        let mut raw = ForeFlightIdMessage {
            device_name: "A".to_string(),
            ..message
        }
        .encode()
        .unwrap();
        raw[13] = b'B';
        assert!(matches!(
            ForeFlightIdMessage::decode(&raw),
            Err(Gdl90Error::InvalidField {
                field: "device name",
                ..
            })
        ));
    }
''',
)

replace_once(
    "src/util.rs",
    '''    if value.len() > N {
        return Err(Gdl90Error::InvalidField {
            field,
            details: format!("string is {} bytes, max is {N}", value.len()),
        });
    }

    let mut out = [0u8; N];''',
    '''    if value.len() > N {
        return Err(Gdl90Error::InvalidField {
            field,
            details: format!("string is {} bytes, max is {N}", value.len()),
        });
    }
    if value.as_bytes().contains(&0) {
        return Err(Gdl90Error::InvalidField {
            field,
            details: "embedded NUL would truncate the fixed-width string".to_string(),
        });
    }

    let mut out = [0u8; N];''',
)

replace_once(
    "src/util.rs",
    '''    let used = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    let text = std::str::from_utf8(&bytes[..used]).map_err(|_| Gdl90Error::Utf8 { field })?;
    Ok(text.to_string())''',
    '''    let used = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    if used < bytes.len() && bytes[used + 1..].iter().any(|byte| *byte != 0) {
        return Err(Gdl90Error::InvalidField {
            field,
            details: "bytes after the first NUL must be zero padding".to_string(),
        });
    }
    let text = std::str::from_utf8(&bytes[..used]).map_err(|_| Gdl90Error::Utf8 { field })?;
    Ok(text.to_string())''',
)

# Garmin sentinel handling and ForeFlight VFOM collision behavior.
replace_once(
    "src/message.rs",
    '''    pub fn encode(&self) -> Result<Vec<u8>> {
        let mut out = Vec::with_capacity(Self::LEN);
        out.push(HEIGHT_ABOVE_TERRAIN_MESSAGE_ID);
        out.extend_from_slice(&self.feet.unwrap_or(i16::MIN).to_be_bytes());
        Ok(out)
    }
}''',
    '''    pub fn encode(&self) -> Result<Vec<u8>> {
        if self.feet == Some(i16::MIN) {
            return Err(Gdl90Error::InvalidField {
                field: "height above terrain",
                details: "-32768 is reserved for an unavailable value".to_string(),
            });
        }
        let mut out = Vec::with_capacity(Self::LEN);
        out.push(HEIGHT_ABOVE_TERRAIN_MESSAGE_ID);
        out.extend_from_slice(&self.feet.unwrap_or(i16::MIN).to_be_bytes());
        Ok(out)
    }
}''',
)

replace_once(
    "src/message.rs",
    '''        let greater_than_sentinel = match encoding {
            VerticalFigureOfMeritEncoding::GarminRevA => Self::GARMIN_GREATER_THAN_32766,
            VerticalFigureOfMeritEncoding::ForeFlightLegacy => Self::FOREFLIGHT_GREATER_THAN_32766,
        };
        let vfom = match self.vertical_figure_of_merit {
            VerticalFigureOfMerit::Meters(value) => value.min(greater_than_sentinel),
            VerticalFigureOfMerit::NotAvailable => Self::NOT_AVAILABLE,
            VerticalFigureOfMerit::GreaterThan32766 => greater_than_sentinel,
        };''',
    '''        let vfom = match (encoding, self.vertical_figure_of_merit) {
            (_, VerticalFigureOfMerit::NotAvailable) => Self::NOT_AVAILABLE,
            (VerticalFigureOfMeritEncoding::GarminRevA, VerticalFigureOfMerit::GreaterThan32766) => {
                Self::GARMIN_GREATER_THAN_32766
            }
            (
                VerticalFigureOfMeritEncoding::ForeFlightLegacy,
                VerticalFigureOfMerit::GreaterThan32766,
            ) => Self::FOREFLIGHT_GREATER_THAN_32766,
            (VerticalFigureOfMeritEncoding::GarminRevA, VerticalFigureOfMerit::Meters(value)) => {
                value.min(Self::GARMIN_GREATER_THAN_32766)
            }
            (
                VerticalFigureOfMeritEncoding::ForeFlightLegacy,
                VerticalFigureOfMerit::Meters(value),
            ) if value == Self::FOREFLIGHT_GREATER_THAN_32766 => {
                return Err(Gdl90Error::InvalidField {
                    field: "ForeFlight VFOM numeric value",
                    details: "32494 meters collides with ForeFlight's published greater-than sentinel"
                        .to_string(),
                });
            }
            (
                VerticalFigureOfMeritEncoding::ForeFlightLegacy,
                VerticalFigureOfMerit::Meters(value),
            ) if value > Self::GARMIN_GREATER_THAN_32766 => {
                Self::FOREFLIGHT_GREATER_THAN_32766
            }
            (
                VerticalFigureOfMeritEncoding::ForeFlightLegacy,
                VerticalFigureOfMerit::Meters(value),
            ) => value,
        };''',
)

# Keep opaque APDU metadata internally consistent by construction.
replace_once(
    "src/uplink.rs",
    '''#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpaqueApdu {
    pub product_id: u16,
    pub descriptor_flags: u8,
    pub raw: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]''',
    '''#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpaqueApdu {
    product_id: u16,
    descriptor_flags: u8,
    raw: Vec<u8>,
}

impl OpaqueApdu {
    pub fn from_raw(raw: Vec<u8>) -> Result<Self> {
        if raw.len() < MIN_APDU_HEADER_LEN || raw.len() > MAX_APDU_LEN {
            return Err(Gdl90Error::InvalidLength {
                context: "opaque APDU",
                expected: "4..=422 bytes",
                actual: raw.len(),
            });
        }
        let descriptor_flags = raw[0] >> 5;
        if descriptor_flags == 0 {
            return Err(Gdl90Error::InvalidField {
                field: "opaque APDU descriptor flags",
                details: "zero flags describe the semantically parsed minimal APDU form".to_string(),
            });
        }
        let product_id = (u16::from(raw[0] & 0x1F) << 6) | u16::from(raw[1] >> 2);
        Ok(Self {
            product_id,
            descriptor_flags,
            raw,
        })
    }

    pub fn product_id(&self) -> u16 {
        self.product_id
    }

    pub fn descriptor_flags(&self) -> u8 {
        self.descriptor_flags
    }

    pub fn raw(&self) -> &[u8] {
        &self.raw
    }

    pub fn into_raw(self) -> Vec<u8> {
        self.raw
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]''',
)

replace_once(
    "src/uplink.rs",
    '''        let descriptor_flags = bytes[0] >> 5;
        let product_id = (u16::from(bytes[0] & 0x1F) << 6) | u16::from(bytes[1] >> 2);
        if descriptor_flags == 0 {
            Ok(Self::Parsed(Apdu::decode(bytes)?))
        } else {
            Ok(Self::OpaqueOptionalDescriptor(OpaqueApdu {
                product_id,
                descriptor_flags,
                raw: bytes.to_vec(),
            }))
        }''',
    '''        let descriptor_flags = bytes[0] >> 5;
        if descriptor_flags == 0 {
            Ok(Self::Parsed(Apdu::decode(bytes)?))
        } else {
            Ok(Self::OpaqueOptionalDescriptor(OpaqueApdu::from_raw(
                bytes.to_vec(),
            )?))
        }''',
)

replace_once(
    "src/uplink.rs",
    '''            Self::Parsed(apdu) => apdu.encode(),
            Self::OpaqueOptionalDescriptor(apdu) => {
                if apdu.raw.len() < MIN_APDU_HEADER_LEN || apdu.raw.len() > MAX_APDU_LEN {
                    return Err(Gdl90Error::InvalidLength {
                        context: "opaque APDU",
                        expected: "4..=422 bytes",
                        actual: apdu.raw.len(),
                    });
                }
                Ok(apdu.raw.clone())
            }''',
    '''            Self::Parsed(apdu) => apdu.encode(),
            Self::OpaqueOptionalDescriptor(apdu) => Ok(apdu.raw().to_vec()),''',
)

replace_once(
    "src/uplink.rs",
    '''            Self::Parsed(apdu) => apdu.header.product_id,
            Self::OpaqueOptionalDescriptor(apdu) => apdu.product_id,''',
    '''            Self::Parsed(apdu) => apdu.header.product_id,
            Self::OpaqueOptionalDescriptor(apdu) => apdu.product_id(),''',
)

# Bound both session reads and persistent capture output.
replace_once(
    "src/session.rs",
    "use std::io::{BufRead, BufReader, BufWriter, Write};",
    "use std::io::{BufRead, BufReader, BufWriter, Read, Write};",
)

replace_once(
    "src/session.rs",
    '''impl SessionReadLimits {
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

#[derive(Debug, Clone, PartialEq, Eq)]''',
    '''impl SessionReadLimits {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionWriteLimits {
    pub max_file_bytes: u64,
    pub max_datagram_bytes: usize,
}

impl Default for SessionWriteLimits {
    fn default() -> Self {
        Self {
            max_file_bytes: DEFAULT_MAX_SESSION_FILE_BYTES,
            max_datagram_bytes: DEFAULT_MAX_DATAGRAM_SIZE,
        }
    }
}

impl SessionWriteLimits {
    fn validate(self) -> Result<Self> {
        if self.max_file_bytes == 0 || self.max_datagram_bytes == 0 {
            return Err(Gdl90Error::InvalidField {
                field: "session write limits",
                details: "all limits must be greater than zero".to_string(),
            });
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]''',
)

replace_once(
    "src/session.rs",
    '''    let mut datagrams = Vec::new();
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
        }''',
    '''    let mut datagrams = Vec::new();
    let mut line = String::new();
    let mut line_number = 0usize;
    let mut total_bytes_read = 0u64;
    let line_read_limit = u64::try_from(limits.max_line_bytes)
        .unwrap_or(u64::MAX)
        .saturating_add(1);
    loop {
        line.clear();
        let bytes_read = reader
            .by_ref()
            .take(line_read_limit)
            .read_line(&mut line)
            .map_err(|error| Gdl90Error::Io {
                context: "read datagram file",
                details: error.to_string(),
            })?;
        if bytes_read == 0 {
            break;
        }
        total_bytes_read = total_bytes_read
            .checked_add(bytes_read as u64)
            .ok_or(Gdl90Error::ResourceLimit {
                resource: "session file bytes",
                limit: usize::try_from(limits.max_file_bytes).unwrap_or(usize::MAX),
            })?;
        if total_bytes_read > limits.max_file_bytes {
            return Err(Gdl90Error::ResourceLimit {
                resource: "session file bytes",
                limit: usize::try_from(limits.max_file_bytes).unwrap_or(usize::MAX),
            });
        }
        line_number += 1;
        if bytes_read > limits.max_line_bytes {
            return Err(Gdl90Error::ResourceLimit {
                resource: "session line bytes",
                limit: limits.max_line_bytes,
            });
        }''',
)

start = read("src/session.rs")
old_start = start.index("pub fn write_datagram_file(")
old_end = start.index("pub fn parse_datagram_line(")
replacement = r'''pub fn write_datagram_file(path: impl AsRef<Path>, datagrams: &[RecordedDatagram]) -> Result<()> {
    write_datagram_file_with_limits(path, datagrams, SessionWriteLimits::default())
}

pub fn write_datagram_file_with_limits(
    path: impl AsRef<Path>,
    datagrams: &[RecordedDatagram],
    limits: SessionWriteLimits,
) -> Result<()> {
    let limits = limits.validate()?;
    let mut lines = Vec::with_capacity(datagrams.len());
    let mut total_bytes = 0u64;
    for datagram in datagrams {
        let line = bounded_datagram_line(datagram, limits.max_datagram_bytes)?;
        let line_bytes = u64::try_from(line.len())
            .unwrap_or(u64::MAX)
            .saturating_add(1);
        total_bytes = total_bytes
            .checked_add(line_bytes)
            .ok_or(Gdl90Error::ResourceLimit {
                resource: "session output bytes",
                limit: usize::try_from(limits.max_file_bytes).unwrap_or(usize::MAX),
            })?;
        if total_bytes > limits.max_file_bytes {
            return Err(Gdl90Error::ResourceLimit {
                resource: "session output bytes",
                limit: usize::try_from(limits.max_file_bytes).unwrap_or(usize::MAX),
            });
        }
        lines.push(line);
    }

    let file = File::create(path.as_ref()).map_err(|error| Gdl90Error::Io {
        context: "create datagram file",
        details: error.to_string(),
    })?;
    let mut writer = BufWriter::new(file);
    for line in lines {
        writeln!(writer, "{line}").map_err(|error| Gdl90Error::Io {
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
    append_datagram_with_limits(path, datagram, SessionWriteLimits::default())
}

pub fn append_datagram_with_limits(
    path: impl AsRef<Path>,
    datagram: &RecordedDatagram,
    limits: SessionWriteLimits,
) -> Result<()> {
    let limits = limits.validate()?;
    let line = bounded_datagram_line(datagram, limits.max_datagram_bytes)?;
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path.as_ref())
        .map_err(|error| Gdl90Error::Io {
            context: "open datagram file for append",
            details: error.to_string(),
        })?;
    let current_bytes = file.metadata().map_err(|error| Gdl90Error::Io {
        context: "read datagram output metadata",
        details: error.to_string(),
    })?.len();
    let line_bytes = u64::try_from(line.len())
        .unwrap_or(u64::MAX)
        .saturating_add(1);
    if current_bytes
        .checked_add(line_bytes)
        .is_none_or(|total| total > limits.max_file_bytes)
    {
        return Err(Gdl90Error::ResourceLimit {
            resource: "session output bytes",
            limit: usize::try_from(limits.max_file_bytes).unwrap_or(usize::MAX),
        });
    }

    let mut writer = BufWriter::new(file);
    writeln!(writer, "{line}").map_err(|error| Gdl90Error::Io {
        context: "append datagram file",
        details: error.to_string(),
    })?;
    writer.flush().map_err(|error| Gdl90Error::Io {
        context: "flush datagram append",
        details: error.to_string(),
    })
}

fn bounded_datagram_line(datagram: &RecordedDatagram, max_datagram_bytes: usize) -> Result<String> {
    if datagram.bytes.len() > max_datagram_bytes {
        return Err(Gdl90Error::ResourceLimit {
            resource: "session datagram bytes",
            limit: max_datagram_bytes,
        });
    }
    Ok(datagram.to_line())
}

'''
write("src/session.rs", start[:old_start] + replacement + start[old_end:])

# Implement the control-panel message cadences described by Garmin.
replace_once(
    "src/control.rs",
    '''use crate::util::{
    decode_ascii_digits, encode_ascii_digits, encode_call_sign as encode_fixed_call_sign,
    hex_checksum, parse_hex_byte,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]''',
    '''use crate::util::{
    decode_ascii_digits, encode_ascii_digits, encode_call_sign as encode_fixed_call_sign,
    hex_checksum, parse_hex_byte,
};
use std::time::Duration;

pub const CONTROL_MODE_INTERVAL: Duration = Duration::from_secs(1);
pub const CONTROL_CALL_SIGN_INTERVAL: Duration = Duration::from_secs(60);
pub const CONTROL_VFR_CODE_INTERVAL: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControlCadenceProfile {
    pub mode_interval: Duration,
    pub call_sign_interval: Duration,
    pub vfr_code_interval: Duration,
}

pub fn cadence_profile() -> ControlCadenceProfile {
    ControlCadenceProfile {
        mode_interval: CONTROL_MODE_INTERVAL,
        call_sign_interval: CONTROL_CALL_SIGN_INTERVAL,
        vfr_code_interval: CONTROL_VFR_CODE_INTERVAL,
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ControlCadenceDue {
    pub mode: bool,
    pub call_sign: bool,
    pub vfr_code: bool,
}

#[derive(Debug, Clone)]
pub struct ControlCadenceScheduler {
    profile: ControlCadenceProfile,
    next_mode: Duration,
    next_call_sign: Duration,
    next_vfr_code: Duration,
    call_sign_changed: bool,
}

impl ControlCadenceScheduler {
    pub fn new(start: Duration) -> Self {
        Self::with_profile(start, cadence_profile())
    }

    pub fn with_profile(start: Duration, profile: ControlCadenceProfile) -> Self {
        Self {
            profile,
            next_mode: start,
            next_call_sign: start,
            next_vfr_code: start,
            call_sign_changed: false,
        }
    }

    pub fn mark_call_sign_changed(&mut self) {
        self.call_sign_changed = true;
    }

    pub fn poll(&mut self, now: Duration) -> ControlCadenceDue {
        let due = ControlCadenceDue {
            mode: now >= self.next_mode,
            call_sign: self.call_sign_changed || now >= self.next_call_sign,
            vfr_code: now >= self.next_vfr_code,
        };
        if due.mode {
            self.next_mode = now.saturating_add(self.profile.mode_interval);
        }
        if due.call_sign {
            self.next_call_sign = now.saturating_add(self.profile.call_sign_interval);
            self.call_sign_changed = false;
        }
        if due.vfr_code {
            self.next_vfr_code = now.saturating_add(self.profile.vfr_code_interval);
        }
        due
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]''',
)

write(
    "src/control.rs",
    read("src/control.rs")
    + r'''

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_cadence_matches_garmin_intervals_and_change_trigger() {
        let mut scheduler = ControlCadenceScheduler::new(Duration::ZERO);
        assert_eq!(
            scheduler.poll(Duration::ZERO),
            ControlCadenceDue {
                mode: true,
                call_sign: true,
                vfr_code: true,
            }
        );
        assert_eq!(
            scheduler.poll(Duration::from_millis(999)),
            ControlCadenceDue::default()
        );
        assert!(scheduler.poll(Duration::from_secs(1)).mode);
        scheduler.mark_call_sign_changed();
        assert!(scheduler.poll(Duration::from_secs(2)).call_sign);
        let minute = scheduler.poll(Duration::from_secs(60));
        assert!(minute.mode);
        assert!(minute.vfr_code);
        assert!(!minute.call_sign);
        assert!(scheduler.poll(Duration::from_secs(62)).call_sign);
    }
}
''',
)

# Make support claims follow what the public documents can actually prove.
support_replacements = [
    (
        '''            "4.3.2",
            "APDU Payload",
            SupportState::Complete,
            "Independent payloads are preserved losslessly and segmented product files are reassembled across uplink messages, including TWGO repeated-header removal and retransmission validation.",''',
        '''            "4.3.2",
            "APDU Payload",
            SupportState::Partial,
            "Independent payload bytes are preserved and bounded reassembly is implemented; complete product-specific payload semantics depend on external FAA/RTCA definitions.",''',
    ),
    (
        '''            "4.4.1",
            "Textual METAR and TAF Products",
            SupportState::Complete,
            "Generic Text APDUs implement the complete six-bit DLAC table, run-length spaces, control boundaries, record packing, and METAR/TAF composition validation.",''',
        '''            "4.4.1",
            "Textual METAR and TAF Products",
            SupportState::BlockedByExternalSpec,
            "The public examples and implemented DLAC codec are tested, but complete product conformance depends on RTCA/DO-267A and the FAA registry.",''',
    ),
    (
        '''            "4.4.2",
            "NEXRAD Graphic Product",
            SupportState::Complete,
            "NEXRAD APDUs decode run-length and empty-element blocks, typed intensity semantics, and geographic block bounds from the block reference indicator.",''',
        '''            "4.4.2",
            "NEXRAD Graphic Product",
            SupportState::BlockedByExternalSpec,
            "The Garmin examples, run-length blocks, and preserved unknown forms are implemented; full Global Block Representation semantics are externally defined.",''',
    ),
    (
        '''            "5.1",
            "Type 4 NEXRAD Precipitation Image – Global Block Representation",
            SupportState::Complete,
            "Type 4 NEXRAD payloads decode run-length and empty-element forms, block-reference scale, and geographic bounds for individual GBR blocks.",''',
        '''            "5.1",
            "Type 4 NEXRAD Precipitation Image – Global Block Representation",
            SupportState::BlockedByExternalSpec,
            "Garmin does not reproduce the complete copyrighted GBR definition; public examples are decoded while unsupported forms remain lossless.",''',
    ),
    (
        '''            "5.1.1",
            "Definition",
            SupportState::Complete,
            "The Global Block Representation carried by the Garmin examples and ETSO amendments is implemented, including scale-aware block geometry.",''',
        '''            "5.1.1",
            "Definition",
            SupportState::BlockedByExternalSpec,
            "The complete Global Block Representation definition is outside the Garmin public ICD.",''',
    ),
    (
        '''            "5.1.3",
            "APDU Payload Format",
            SupportState::Complete,
            "The documented APDU header constraints, block reference indicator, run-length blocks, and empty-element bitmap form are implemented.",''',
        '''            "5.1.3",
            "APDU Payload Format",
            SupportState::BlockedByExternalSpec,
            "Public header and example forms are implemented, but the complete payload format is delegated to external specifications.",''',
    ),
    (
        '''            "5.2",
            "Generic Textual Data Product – Type 2 (DLAC)",
            SupportState::Complete,
            "Generic Text records, exact six-bit DLAC packing, code-28 run-length spaces, control characters, whole-record APDU packing, and METAR/TAF composition are implemented.",''',
        '''            "5.2",
            "Generic Textual Data Product – Type 2 (DLAC)",
            SupportState::BlockedByExternalSpec,
            "The public examples and implemented record codec are tested; the normative DLAC/product definitions remain external.",''',
    ),
    (
        '''            "5.2.1",
            "Definition",
            SupportState::Complete,
            "The Generic Text Type 2 record model and every DLAC code position used by the FIS-B profile are implemented.",''',
        '''            "5.2.1",
            "Definition",
            SupportState::BlockedByExternalSpec,
            "The complete Generic Text Type 2 and DLAC definitions are referenced to external RTCA material.",''',
    ),
    (
        '''            "5.2.2",
            "APDU Payload Format",
            SupportState::Complete,
            "Generic Text APDU payload packing and decoding implement ETX padding, SUB/NC handling, record separators, line feeds, pipe, printable characters, and run-length spaces.",''',
        '''            "5.2.2",
            "APDU Payload Format",
            SupportState::BlockedByExternalSpec,
            "The implemented payload codec covers public examples, but complete normative character and packing behavior is externally controlled.",''',
    ),
    (
        '''            "5.2.3",
            "METAR / TAF Composition",
            SupportState::Complete,
            "METAR/TAF token structure, qualifiers, NIL handling, non-empty report text, and whole-record validation are implemented.",''',
        '''            "5.2.3",
            "METAR / TAF Composition",
            SupportState::Partial,
            "The public token structure and examples are validated; the FAA product registry remains authoritative for complete composition rules.",''',
    ),
    (
        '''            "6",
            "Control Panel Interface",
            SupportState::Partial,
            "ASCII message codecs and cadence metadata are implemented; physical serial transport and certified equipment integration are not provided.",''',
        '''            "6",
            "Control Panel Interface",
            SupportState::Partial,
            "ASCII codecs and deterministic one-second/one-minute cadence scheduling are implemented; physical serial transport and certified equipment integration are not provided.",''',
    ),
    (
        '''            "ForeFlight Messages",
            "ForeFlight Message Set",
            SupportState::Complete,
            "The documented supported-message subset and UDP datagram encoding rules are implemented.",''',
        '''            "ForeFlight Messages",
            "ForeFlight Message Set",
            SupportState::Partial,
            "The supported subset and packet rules are implemented, with explicit compatibility handling for published VFOM and heading ambiguities.",''',
    ),
    (
        '''            "ForeFlight Geo Altitude",
            "ForeFlight Ownship Geometric Altitude",
            SupportState::Complete,
            "Ownship geometric altitude is supported through the core GDL90 codec and the ForeFlight capabilities mask handling.",''',
        '''            "ForeFlight Geo Altitude",
            "ForeFlight Ownship Geometric Altitude",
            SupportState::Partial,
            "Garmin and ForeFlight publish conflicting greater-than VFOM sentinels; strict and opt-in compatibility modes are implemented pending device interoperability evidence.",''',
    ),
    (
        '''            "ForeFlight AHRS",
            "ForeFlight AHRS Message",
            SupportState::Complete,
            "AHRS encode/decode covers roll, pitch, heading type, IAS, TAS, invalid sentinels, and range validation.",''',
        '''            "ForeFlight AHRS",
            "ForeFlight AHRS Message",
            SupportState::Partial,
            "Roll, pitch, heading type, airspeeds, sentinels, and cadence are implemented. Negative heading inputs are angle-canonicalized because the published wire field does not define a signed encoding.",''',
    ),
]
for old, new in support_replacements:
    replace_once("src/support.rs", old, new)

replace_once(
    "src/support.rs",
    '''        for section in ["2.1", "3.6", "4.3.1", "4.4", "6.1"] {
            assert!(missing.iter().any(|entry| entry.section == section));
        }''',
    '''        for section in [
            "2.1",
            "3.6",
            "4.3.1",
            "4.3.2",
            "4.4",
            "4.4.1",
            "4.4.2",
            "5.1",
            "5.2",
            "6.1",
            "ForeFlight Geo Altitude",
            "ForeFlight AHRS",
        ] {
            assert!(missing.iter().any(|entry| entry.section == section));
        }''',
)

# Public descriptions and report naming must match the actual guarantees.
replace_once(
    "src/lib.rs",
    "//! - Async HDLC framing with CRC-CCITT FCS and byte stuffing.",
    "//! - HDLC-style framing with CRC-CCITT FCS and byte stuffing.",
)
replace_once(
    "src/report.rs",
    "use crate::analysis::{SessionAnalysis, SessionValidation, analyze_datagrams, validate_datagrams};",
    "use crate::analysis::{SessionAnalysis, SessionValidation, analyze_datagrams, validate_datagrams_syntax};",
)
replace_once(
    "src/report.rs",
    "    let validation = validate_datagrams(datagrams);",
    "    let validation = validate_datagrams_syntax(datagrams);",
)

readme_replacements = [
    (
        "  - APDU headers, civil-time variants, reserved-bit validation, and segmentation metadata",
        "  - minimal APDU headers, civil-time variants, and segmentation metadata",
    ),
    (
        "  - stateful product-file reassembly across uplink messages",
        "  - bounded, source- and generation-scoped product-file reassembly",
    ),
    (
        "  - named, lossless raw preservation for product schemas outside Garmin Rev A",
        "  - lossless raw preservation for optional descriptors and product schemas outside Garmin Rev A",
    ),
    (
        "  - datagram validation with issue reporting",
        "  - syntactic datagram validation with issue reporting",
    ),
    (
        "  - control-panel serial profiles and connector mapping",
        "  - control-panel serial profiles, connector mapping, and cadence scheduling",
    ),
    (
        '''cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings''',
        '''cargo fmt --all -- --check
cargo fmt --manifest-path fuzz/Cargo.toml --all -- --check
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
cargo check --manifest-path fuzz/Cargo.toml --all-targets''',
    ),
    (
        "- independent decoding for each UDP datagram and source-safe APDU reassembly APIs",
        "- independent decoding for each UDP datagram and source-scoped APDU reassembly APIs",
    ),
    (
        "- IP-version-aware ForeFlight UDP payload limits and a deterministic 5 Hz AHRS cadence scheduler",
        "- IP-version-aware ForeFlight UDP payload limits plus deterministic AHRS, discovery, and control-panel cadence schedulers",
    ),
]
for old, new in readme_replacements:
    replace_once("README.md", old, new)

conformance = read("docs/CONFORMANCE.md").lstrip()
conformance = conformance.replace(
    "- APDU reassembly is bounded by file count, byte count, segment count, age, and caller-supplied source identity.",
    "- APDU reassembly is bounded by file count, byte count, segment count, age, source identity, and APDU generation time.",
)
conformance = conformance.replace(
    "Garmin Rev A assigns `0x7FFE` to geometric VFOM greater than 32766 m. ForeFlight's published page currently shows `0x7EEE`. Strict decoding follows Garmin. `decode_foreflight_compatible` and `encode_for_foreflight` make the conflicting legacy behavior opt-in and testable.",
    "Garmin Rev A assigns `0x7FFE` to geometric VFOM greater than 32766 m. ForeFlight's published page currently shows `0x7EEE`. Strict decoding follows Garmin. `decode_foreflight_compatible` and `encode_for_foreflight` make the conflicting legacy behavior opt-in and reject numeric values that collide with the ForeFlight sentinel.\n\nForeFlight publishes an AHRS heading input range of -360.0 through +360.0 degrees while allocating bits 14–0 to the heading value without defining a signed representation. Encoding accepts that published API range and canonicalizes negative angles to their equivalent positive heading; decoding returns the canonical nonnegative wire value. This behavior remains marked `Partial` until representative-device interoperability confirms the interpretation.",
)
write("docs/CONFORMANCE.md", conformance)

readiness = read("docs/PRODUCTION_READINESS.md").lstrip()
readiness = readiness.replace(
    "- reassembly source identifiers derived from a stable station or transport identity",
    "- reassembly source identifiers derived from a stable station or transport identity\n- capture-file rotation, retention, disk quotas, and atomic export publication beyond the crate's default file-size guard\n- explicit text/JSON report-output budgets appropriate for the embedding process",
)
write("docs/PRODUCTION_READINESS.md", readiness)

# Update regression tests for immutable opaque values and the newly closed sentinels.
replace_once(
    "tests/protocol.rs",
    '''        ApduPayload::OpaqueOptionalDescriptor(opaque) => {
            assert_eq!(opaque.descriptor_flags, 0b101);
            assert_eq!(opaque.raw, raw);
        }''',
    '''        ApduPayload::OpaqueOptionalDescriptor(opaque) => {
            assert_eq!(opaque.descriptor_flags(), 0b101);
            assert_eq!(opaque.raw(), raw.as_slice());
        }''',
)

write(
    "tests/protocol.rs",
    read("tests/protocol.rs")
    + r'''

#[test]
fn unavailable_sentinels_cannot_be_constructed_as_real_garmin_values() {
    let height = HeightAboveTerrain {
        feet: Some(i16::MIN),
    };
    assert!(matches!(
        height.encode(),
        Err(gdl90::Gdl90Error::InvalidField {
            field: "height above terrain",
            ..
        })
    ));
}

#[test]
fn foreflight_vfom_compatibility_preserves_noncolliding_numeric_values() {
    let numeric = OwnshipGeometricAltitude {
        altitude_feet: 0,
        vertical_warning: false,
        vertical_figure_of_merit: VerticalFigureOfMerit::Meters(0x7EEF),
    };
    let encoded = numeric.encode_for_foreflight().unwrap();
    assert_eq!(u16::from_be_bytes([encoded[3], encoded[4]]) & 0x7FFF, 0x7EEF);

    let collision = OwnshipGeometricAltitude {
        vertical_figure_of_merit: VerticalFigureOfMerit::Meters(0x7EEE),
        ..numeric
    };
    assert!(matches!(
        collision.encode_for_foreflight(),
        Err(gdl90::Gdl90Error::InvalidField {
            field: "ForeFlight VFOM numeric value",
            ..
        })
    ));
}

#[test]
fn opaque_apdu_constructor_rejects_the_minimal_descriptor_form() {
    assert!(matches!(
        gdl90::OpaqueApdu::from_raw(vec![0, 0, 0, 0]),
        Err(gdl90::Gdl90Error::InvalidField {
            field: "opaque APDU descriptor flags",
            ..
        })
    ));
}
''',
)

replace_once(
    "tests/session.rs",
    '''use gdl90::session::{
    RecordedDatagram, append_datagram, parse_datagram_line, read_datagram_file, write_datagram_file,
};''',
    '''use gdl90::session::{
    RecordedDatagram, SessionReadLimits, SessionWriteLimits, append_datagram,
    append_datagram_with_limits, parse_datagram_line, read_datagram_file,
    read_datagram_file_with_limits, write_datagram_file,
};''',
)

write(
    "tests/session.rs",
    read("tests/session.rs")
    + r'''

#[test]
fn session_read_limits_stop_before_unbounded_line_allocation() {
    let path = std::env::temp_dir().join(format!(
        "gdl90-long-session-{}.txt",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(&path, "00112233445566778899\n").unwrap();
    let error = read_datagram_file_with_limits(
        &path,
        SessionReadLimits {
            max_file_bytes: 1024,
            max_line_bytes: 8,
            max_datagrams: 10,
            max_datagram_bytes: 32,
        },
    )
    .unwrap_err();
    assert!(matches!(
        error,
        Gdl90Error::ResourceLimit {
            resource: "session line bytes",
            limit: 8
        }
    ));
    let _ = std::fs::remove_file(path);
}

#[test]
fn session_append_enforces_datagram_and_file_limits() {
    let path = std::env::temp_dir().join(format!(
        "gdl90-bounded-output-{}.txt",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let datagram = RecordedDatagram {
        delay_ms: None,
        bytes: vec![0x7E, 0x00, 0x7E],
    };
    let limits = SessionWriteLimits {
        max_file_bytes: 8,
        max_datagram_bytes: 3,
    };
    append_datagram_with_limits(&path, &datagram, limits).unwrap();
    assert!(matches!(
        append_datagram_with_limits(&path, &datagram, limits),
        Err(Gdl90Error::ResourceLimit {
            resource: "session output bytes",
            limit: 8
        })
    ));

    let oversized = RecordedDatagram {
        delay_ms: None,
        bytes: vec![0; 4],
    };
    assert!(matches!(
        append_datagram_with_limits(&path, &oversized, limits),
        Err(Gdl90Error::ResourceLimit {
            resource: "session datagram bytes",
            limit: 3
        })
    ));
    let _ = std::fs::remove_file(path);
}
''',
)
