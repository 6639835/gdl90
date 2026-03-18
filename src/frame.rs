use crate::error::{Gdl90Error, Result};

pub const FLAG_BYTE: u8 = 0x7E;
pub const ESCAPE_BYTE: u8 = 0x7D;
pub const ESCAPE_MASK: u8 = 0x20;

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

#[derive(Debug, Default, Clone)]
pub struct FrameDecoder {
    collecting: bool,
    buffer: Vec<u8>,
}

impl FrameDecoder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, bytes: &[u8]) -> Vec<Result<Vec<u8>>> {
        let mut frames = Vec::new();

        for byte in bytes {
            if *byte == FLAG_BYTE {
                if self.collecting && !self.buffer.is_empty() {
                    let stuffed = std::mem::take(&mut self.buffer);
                    frames.push(decode_clear_message(&match unescape(&stuffed) {
                        Ok(data) => data,
                        Err(error) => {
                            frames.push(Err(error));
                            self.buffer.clear();
                            self.collecting = true;
                            continue;
                        }
                    }));
                } else {
                    self.buffer.clear();
                }
                self.collecting = true;
            } else if self.collecting {
                self.buffer.push(*byte);
            }
        }

        frames
    }

    pub fn reset(&mut self) {
        self.collecting = false;
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
