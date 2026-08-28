use crate::error::{Gdl90Error, Result};
use crate::util::{
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlMode {
    Standby,
    ModeA,
    ModeC,
}

impl ControlMode {
    fn from_raw(raw: u8) -> Result<Self> {
        match raw {
            b'O' => Ok(Self::Standby),
            b'A' => Ok(Self::ModeA),
            b'C' => Ok(Self::ModeC),
            _ => Err(Gdl90Error::ControlFormat("unknown control mode")),
        }
    }

    fn raw(self) -> u8 {
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
    fn from_raw(raw: u8) -> Result<Self> {
        match raw {
            b'I' => Ok(Self::Active),
            b'-' => Ok(Self::Inactive),
            _ => Err(Gdl90Error::ControlFormat("unknown ident status")),
        }
    }

    fn raw(self) -> u8 {
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
    fn from_raw(raw: u8) -> Result<Self> {
        match raw {
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

    fn raw(self) -> u8 {
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
    let call_sign = decode_control_callsign(&line[4..12])?;
    Ok(CallSignMessage { call_sign })
}

fn encode_call_sign(message: &CallSignMessage) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(15);
    out.extend_from_slice(b"^CS ");
    out.extend_from_slice(&encode_fixed_call_sign(&message.call_sign)?);
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
    let mode = ControlMode::from_raw(line[4])?;
    let ident = IdentStatus::from_raw(line[6])?;
    let squawk = decode_ascii_digits(&line[8..12], 4, "squawk")
        .map_err(|_| control_digit_format("squawk"))?;
    let emergency = EmergencyCode::from_raw(line[12])?;
    match line[13] {
        b'1' => {}
        _ => return Err(Gdl90Error::ControlFormat("health bit must be '1'")),
    }
    Ok(ModeMessage {
        mode,
        ident,
        squawk,
        emergency,
    })
}

fn encode_mode(message: &ModeMessage) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(17);
    out.extend_from_slice(b"^MD ");
    out.push(message.mode.raw());
    out.push(b',');
    out.push(message.ident.raw());
    out.push(b',');
    out.extend_from_slice(&encode_ascii_digits(&message.squawk, 4, "squawk")?);
    out.push(message.emergency.raw());
    out.push(b'1');
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
    let vfr_code = decode_ascii_digits(&line[4..8], 4, "VFR code")
        .map_err(|_| control_digit_format("VFR code"))?;
    Ok(VfrCodeMessage { vfr_code })
}

fn control_digit_format(field: &'static str) -> Gdl90Error {
    match field {
        "squawk" => Gdl90Error::ControlFormat("squawk must be 4 digits"),
        "VFR code" => Gdl90Error::ControlFormat("VFR code must be 4 digits"),
        _ => Gdl90Error::ControlFormat("invalid fixed-width digit field"),
    }
}

fn decode_control_callsign(bytes: &[u8]) -> Result<String> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| Gdl90Error::ControlFormat("call sign must be ASCII"))?;
    if !text
        .bytes()
        .all(|byte| matches!(byte, b'0'..=b'9' | b'A'..=b'Z' | b' ' | b'-'))
    {
        return Err(Gdl90Error::ControlFormat(
            "call sign contains unsupported characters",
        ));
    }
    let first_pad = bytes.iter().position(|byte| *byte == b' ');
    if let Some(first_pad) = first_pad
        && bytes[first_pad..].iter().any(|byte| *byte != b' ')
    {
        return Err(Gdl90Error::ControlFormat(
            "call sign spaces must be trailing padding",
        ));
    }
    Ok(text.trim_end().to_string())
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
