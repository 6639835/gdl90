use crate::error::{Gdl90Error, Result};

pub(crate) fn read_be_i24(bytes: &[u8]) -> i32 {
    let value = ((bytes[0] as i32) << 16) | ((bytes[1] as i32) << 8) | bytes[2] as i32;
    if (value & 0x80_0000) != 0 {
        value | !0x00FF_FFFF
    } else {
        value
    }
}

pub(crate) fn write_be_i24(value: i32) -> Result<[u8; 3]> {
    if !(-8_388_608..=8_388_607).contains(&value) {
        return Err(Gdl90Error::InvalidField {
            field: "24-bit signed integer",
            details: format!("{value} is out of range"),
        });
    }

    Ok([
        ((value >> 16) & 0xFF) as u8,
        ((value >> 8) & 0xFF) as u8,
        (value & 0xFF) as u8,
    ])
}

pub(crate) fn write_le_u24(value: u32) -> Result<[u8; 3]> {
    if value > 0xFF_FFFF {
        return Err(Gdl90Error::InvalidField {
            field: "24-bit unsigned integer",
            details: format!("{value} is out of range"),
        });
    }

    Ok([
        (value & 0xFF) as u8,
        ((value >> 8) & 0xFF) as u8,
        ((value >> 16) & 0xFF) as u8,
    ])
}

pub(crate) fn read_le_u24(bytes: &[u8]) -> u32 {
    bytes[0] as u32 | ((bytes[1] as u32) << 8) | ((bytes[2] as u32) << 16)
}

pub(crate) fn read_be_i16(bytes: &[u8]) -> i16 {
    i16::from_be_bytes([bytes[0], bytes[1]])
}

pub(crate) fn encode_fixed_utf8<const N: usize>(
    value: &str,
    field: &'static str,
) -> Result<[u8; N]> {
    if value.len() > N {
        return Err(Gdl90Error::InvalidField {
            field,
            details: format!("string is {} bytes, max is {N}", value.len()),
        });
    }

    let mut out = [0u8; N];
    out[..value.len()].copy_from_slice(value.as_bytes());
    Ok(out)
}

pub(crate) fn decode_fixed_utf8(bytes: &[u8], field: &'static str) -> Result<String> {
    let used = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    let text = std::str::from_utf8(&bytes[..used]).map_err(|_| Gdl90Error::Utf8 { field })?;
    Ok(text.to_string())
}

pub(crate) fn encode_callsign(value: &str) -> Result<[u8; 8]> {
    let uppercase = value.to_ascii_uppercase();
    if uppercase.len() > 8 {
        return Err(Gdl90Error::InvalidField {
            field: "call sign",
            details: format!("string is {} bytes, max is 8", uppercase.len()),
        });
    }

    let mut out = [b' '; 8];
    for (index, byte) in uppercase.bytes().enumerate() {
        let valid = matches!(byte, b'0'..=b'9' | b'A'..=b'Z' | b' ' | b'-');
        if !valid {
            return Err(Gdl90Error::InvalidField {
                field: "call sign",
                details: format!("byte {byte:#04x} is not permitted"),
            });
        }
        out[index] = byte;
    }

    Ok(out)
}

pub(crate) fn decode_callsign(bytes: &[u8; 8]) -> String {
    let end = bytes
        .iter()
        .rposition(|byte| *byte != b' ')
        .map(|index| index + 1)
        .unwrap_or(0);
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

pub(crate) fn encode_ascii_digits(
    value: &str,
    width: usize,
    field: &'static str,
) -> Result<Vec<u8>> {
    if value.len() != width || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(Gdl90Error::InvalidField {
            field,
            details: format!("must be exactly {width} ASCII digits"),
        });
    }

    Ok(value.as_bytes().to_vec())
}

pub(crate) fn hex_checksum(data: &[u8]) -> [u8; 2] {
    let sum = data.iter().fold(0u8, |acc, byte| acc.wrapping_add(*byte));
    let hi = nibble_to_hex(sum >> 4);
    let lo = nibble_to_hex(sum & 0x0F);
    [hi, lo]
}

pub(crate) fn parse_hex_byte(bytes: &[u8], context: &'static str) -> Result<u8> {
    if bytes.len() != 2 {
        return Err(Gdl90Error::ControlFormat(context));
    }

    let hi = hex_to_nibble(bytes[0]).ok_or(Gdl90Error::ControlFormat(context))?;
    let lo = hex_to_nibble(bytes[1]).ok_or(Gdl90Error::ControlFormat(context))?;
    Ok((hi << 4) | lo)
}

fn nibble_to_hex(nibble: u8) -> u8 {
    match nibble {
        0..=9 => b'0' + nibble,
        10..=15 => b'A' + (nibble - 10),
        _ => unreachable!(),
    }
}

fn hex_to_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'A'..=b'F' => Some(value - b'A' + 10),
        b'a'..=b'f' => Some(value - b'a' + 10),
        _ => None,
    }
}

pub(crate) fn lat_lon_to_degrees(raw: i32) -> f64 {
    raw as f64 * 180.0 / 8_388_608.0
}

pub(crate) fn degrees_to_lat_lon(
    degrees: f64,
    field: &'static str,
    min: f64,
    max: f64,
) -> Result<[u8; 3]> {
    if !(min..=max).contains(&degrees) {
        return Err(Gdl90Error::InvalidField {
            field,
            details: format!("{degrees} is outside [{min}, {max}]"),
        });
    }

    let raw = (degrees * 8_388_608.0 / 180.0).round() as i32;
    write_be_i24(raw)
}
