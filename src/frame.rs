use crate::error::{Gdl90Error, Result};

pub const FLAG_BYTE: u8 = 0x7E;
pub const ESCAPE_BYTE: u8 = 0x7D;
pub const ESCAPE_MASK: u8 = 0x20;

/// Safely exceeds the worst-case stuffed size of every fixed-length message
/// implemented by this crate while preventing an unterminated frame from
/// growing memory without bound.
pub const DEFAULT_MAX_STUFFED_FRAME_LEN: usize = 1_024;

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

#[derive(Debug, Clone)]
pub struct FrameDecoder {
    collecting: bool,
    discarding_oversized: bool,
    buffer: Vec<u8>,
    max_stuffed_frame_len: usize,
}

impl Default for FrameDecoder {
    fn default() -> Self {
        Self {
            collecting: false,
            discarding_oversized: false,
            buffer: Vec::new(),
            max_stuffed_frame_len: DEFAULT_MAX_STUFFED_FRAME_LEN,
        }
    }
}

impl FrameDecoder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_max_stuffed_frame_len(max_stuffed_frame_len: usize) -> Result<Self> {
        if max_stuffed_frame_len < 3 {
            return Err(Gdl90Error::InvalidField {
                field: "maximum stuffed frame length",
                details: "must allow at least one payload byte and the two-byte FCS".to_string(),
            });
        }
        Ok(Self {
            max_stuffed_frame_len,
            ..Self::default()
        })
    }

    pub fn max_stuffed_frame_len(&self) -> usize {
        self.max_stuffed_frame_len
    }

    pub fn push(&mut self, bytes: &[u8]) -> Vec<Result<Vec<u8>>> {
        let mut frames = Vec::new();

        for byte in bytes {
            if *byte == FLAG_BYTE {
                if self.discarding_oversized {
                    frames.push(Err(Gdl90Error::FrameTooLong {
                        limit: self.max_stuffed_frame_len,
                    }));
                    self.discarding_oversized = false;
                    self.buffer.clear();
                    self.collecting = false;
                } else if self.collecting && !self.buffer.is_empty() {
                    let stuffed = std::mem::take(&mut self.buffer);
                    match unescape(&stuffed) {
                        Ok(data) => frames.push(decode_clear_message(&data)),
                        Err(error) => frames.push(Err(error)),
                    }
                    self.collecting = false;
                } else {
                    self.buffer.clear();
                    self.collecting = true;
                }
            } else if self.collecting && !self.discarding_oversized {
                if self.buffer.len() >= self.max_stuffed_frame_len {
                    self.buffer.clear();
                    self.discarding_oversized = true;
                } else {
                    self.buffer.push(*byte);
                }
            }
        }

        frames
    }

    pub fn finish(&mut self) -> Option<Result<Vec<u8>>> {
        let result = if self.discarding_oversized {
            Some(Err(Gdl90Error::FrameTooLong {
                limit: self.max_stuffed_frame_len,
            }))
        } else if self.collecting && !self.buffer.is_empty() {
            Some(Err(Gdl90Error::FrameTooShort))
        } else {
            None
        };
        self.reset();
        result
    }

    pub fn reset(&mut self) {
        self.collecting = false;
        self.discarding_oversized = false;
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

#[cfg(test)]
mod tests {
    use super::{FrameDecoder, decode_frame, encode_frame};
    use crate::Gdl90Error;

    #[test]
    fn byte_stuffing_examples_match_public_icd_examples() {
        let stuffed_flag = encode_frame(&[0x02, 0x7E]);
        assert_eq!(&stuffed_flag[..4], &[0x7E, 0x02, 0x7D, 0x5E]);
        assert_eq!(decode_frame(&stuffed_flag).unwrap(), vec![0x02, 0x7E]);

        let stuffed_escape = encode_frame(&[0x03, 0x7D]);
        assert_eq!(&stuffed_escape[..4], &[0x7E, 0x03, 0x7D, 0x5D]);
        assert_eq!(decode_frame(&stuffed_escape).unwrap(), vec![0x03, 0x7D]);

        let stuffed_crc = encode_frame(&[0x7E, 0x7D]);
        assert!(stuffed_crc.ends_with(&[0x7D, 0x5D, 0x7D, 0x5E, 0x7E]));
        assert_eq!(decode_frame(&stuffed_crc).unwrap(), vec![0x7E, 0x7D]);
    }

    #[test]
    fn oversized_stream_frame_is_bounded_and_decoder_recovers() {
        let mut decoder = FrameDecoder::with_max_stuffed_frame_len(8).unwrap();
        let mut bytes = vec![0x7E];
        bytes.extend_from_slice(&[0; 9]);
        bytes.push(0x7E);
        bytes.extend_from_slice(&encode_frame(&[0x02, 0x00, 0x00]));

        let decoded = decoder.push(&bytes);
        assert!(matches!(
            decoded.first(),
            Some(Err(Gdl90Error::FrameTooLong { limit: 8 }))
        ));
        assert_eq!(decoded[1].as_ref().unwrap(), &[0x02, 0x00, 0x00]);
    }

    #[test]
    fn unfinished_oversized_frame_reports_limit_on_finish() {
        let mut decoder = FrameDecoder::with_max_stuffed_frame_len(4).unwrap();
        decoder.push(&[0x7E, 1, 2, 3, 4, 5]);
        assert!(matches!(
            decoder.finish(),
            Some(Err(Gdl90Error::FrameTooLong { limit: 4 }))
        ));
    }
}
