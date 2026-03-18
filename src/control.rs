use crate::error::{Gdl90Error, Result};
use crate::util::{encode_ascii_digits, encode_callsign, hex_checksum, parse_hex_byte};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlMode {
    Standby,
    ModeA,
    ModeC,
}

impl ControlMode {
    fn from_byte(byte: u8) -> Result<Self> {
        match byte {
            b'O' => Ok(Self::Standby),
            b'A' => Ok(Self::ModeA),
            b'C' => Ok(Self::ModeC),
            _ => Err(Gdl90Error::ControlFormat("unknown control mode")),
        }
    }

    fn byte(self) -> u8 {
        match self {
            Self::Standby => b'O',
            Self::ModeA => b'A',
            Self::ModeC => b'C',
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentStatus {
    Active,
    Inactive,
}

impl IdentStatus {
    fn from_byte(byte: u8) -> Result<Self> {
        match byte {
            b'I' => Ok(Self::Active),
            b'-' => Ok(Self::Inactive),
            _ => Err(Gdl90Error::ControlFormat("unknown ident status")),
        }
    }

    fn byte(self) -> u8 {
        match self {
            Self::Active => b'I',
            Self::Inactive => b'-',
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmergencyCode {
    None,
    General,
    Medical,
    Fuel,
    Communication,
    Hijack,
    Downed,
}

impl EmergencyCode {
    fn from_byte(byte: u8) -> Result<Self> {
        match byte {
            b'0' => Ok(Self::None),
            b'1' => Ok(Self::General),
            b'2' => Ok(Self::Medical),
            b'3' => Ok(Self::Fuel),
            b'4' => Ok(Self::Communication),
            b'5' => Ok(Self::Hijack),
            b'6' => Ok(Self::Downed),
            _ => Err(Gdl90Error::ControlFormat("unknown emergency code")),
        }
    }

    fn byte(self) -> u8 {
        match self {
            Self::None => b'0',
            Self::General => b'1',
            Self::Medical => b'2',
            Self::Fuel => b'3',
            Self::Communication => b'4',
            Self::Hijack => b'5',
            Self::Downed => b'6',
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallSignMessage {
    pub call_sign: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModeMessage {
    pub mode: ControlMode,
    pub ident: IdentStatus,
    pub squawk: String,
    pub emergency: EmergencyCode,
    pub healthy: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VfrCodeMessage {
    pub vfr_code: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlMessage {
    CallSign(CallSignMessage),
    Mode(ModeMessage),
    VfrCode(VfrCodeMessage),
}

impl ControlMessage {
    pub fn decode(line: &[u8]) -> Result<Self> {
        if !line.ends_with(b"\r") {
            return Err(Gdl90Error::ControlFormat(
                "message must end with carriage return",
            ));
        }
        if line.len() < 6 || line[0] != b'^' {
            return Err(Gdl90Error::ControlFormat("message must start with '^'"));
        }

        match &line[0..3] {
            b"^CS" => Ok(Self::CallSign(decode_call_sign(line)?)),
            b"^MD" => Ok(Self::Mode(decode_mode(line)?)),
            b"^VC" => Ok(Self::VfrCode(decode_vfr_code(line)?)),
            _ => Err(Gdl90Error::ControlFormat("unknown control message id")),
        }
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        match self {
            Self::CallSign(message) => encode_call_sign(message),
            Self::Mode(message) => encode_mode(message),
            Self::VfrCode(message) => encode_vfr_code(message),
        }
    }
}

fn decode_call_sign(line: &[u8]) -> Result<CallSignMessage> {
    if line.len() != 15 || line[3] != b' ' {
        return Err(Gdl90Error::ControlFormat(
            "call sign message must be 15 bytes",
        ));
    }
    verify_checksum(line, 12, 12..14)?;
    let call_sign = String::from_utf8_lossy(&line[4..12]).trim_end().to_string();
    Ok(CallSignMessage { call_sign })
}

fn encode_call_sign(message: &CallSignMessage) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(15);
    out.extend_from_slice(b"^CS ");
    out.extend_from_slice(&encode_callsign(&message.call_sign)?);
    let checksum = hex_checksum(&out);
    out.extend_from_slice(&checksum);
    out.push(b'\r');
    Ok(out)
}

fn decode_mode(line: &[u8]) -> Result<ModeMessage> {
    if line.len() != 17 || line[3] != b' ' || line[5] != b',' || line[7] != b',' {
        return Err(Gdl90Error::ControlFormat("mode message must be 17 bytes"));
    }
    verify_checksum(line, 14, 14..16)?;
    let mode = ControlMode::from_byte(line[4])?;
    let ident = IdentStatus::from_byte(line[6])?;
    let squawk = String::from_utf8_lossy(&line[8..12]).to_string();
    if !squawk.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(Gdl90Error::ControlFormat("squawk must be 4 digits"));
    }
    let emergency = EmergencyCode::from_byte(line[12])?;
    let healthy = match line[13] {
        b'1' => true,
        _ => return Err(Gdl90Error::ControlFormat("health bit must be '1'")),
    };
    Ok(ModeMessage {
        mode,
        ident,
        squawk,
        emergency,
        healthy,
    })
}

fn encode_mode(message: &ModeMessage) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(17);
    out.extend_from_slice(b"^MD ");
    out.push(message.mode.byte());
    out.push(b',');
    out.push(message.ident.byte());
    out.push(b',');
    out.extend_from_slice(&encode_ascii_digits(&message.squawk, 4, "squawk")?);
    out.push(message.emergency.byte());
    out.push(if message.healthy { b'1' } else { b'0' });
    let checksum = hex_checksum(&out);
    out.extend_from_slice(&checksum);
    out.push(b'\r');
    Ok(out)
}

fn decode_vfr_code(line: &[u8]) -> Result<VfrCodeMessage> {
    if line.len() != 11 || line[3] != b' ' {
        return Err(Gdl90Error::ControlFormat(
            "VFR code message must be 11 bytes",
        ));
    }
    verify_checksum(line, 8, 8..10)?;
    let vfr_code = String::from_utf8_lossy(&line[4..8]).to_string();
    if !vfr_code.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(Gdl90Error::ControlFormat("VFR code must be 4 digits"));
    }
    Ok(VfrCodeMessage { vfr_code })
}

fn encode_vfr_code(message: &VfrCodeMessage) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(11);
    out.extend_from_slice(b"^VC ");
    out.extend_from_slice(&encode_ascii_digits(&message.vfr_code, 4, "VFR code")?);
    let checksum = hex_checksum(&out);
    out.extend_from_slice(&checksum);
    out.push(b'\r');
    Ok(out)
}

fn verify_checksum(
    line: &[u8],
    checked_len: usize,
    checksum_range: std::ops::Range<usize>,
) -> Result<()> {
    let expected = line[..checked_len]
        .iter()
        .fold(0u8, |acc, byte| acc.wrapping_add(*byte));
    let actual = parse_hex_byte(&line[checksum_range], "invalid checksum field")?;
    if expected != actual {
        return Err(Gdl90Error::ControlChecksumMismatch { expected, actual });
    }
    Ok(())
}
