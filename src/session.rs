use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;

use crate::error::{Gdl90Error, Result};
use crate::message::{FrameMessageDecoder, Message};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedDatagram {
    pub delay_ms: Option<u64>,
    pub bytes: Vec<u8>,
}

impl RecordedDatagram {
    pub fn decode_messages(&self) -> Vec<Result<Message>> {
        let mut decoder = FrameMessageDecoder::new();
        decoder.push(&self.bytes)
    }

    pub fn to_line(&self) -> String {
        let hex = encode_hex(&self.bytes);
        match self.delay_ms {
            Some(delay_ms) => format!("@{delay_ms} {hex}"),
            None => hex,
        }
    }
}

pub fn read_datagram_file(path: impl AsRef<Path>) -> Result<Vec<RecordedDatagram>> {
    let file = File::open(path.as_ref()).map_err(|error| Gdl90Error::Io {
        context: "open datagram file",
        details: error.to_string(),
    })?;
    let reader = BufReader::new(file);
    let mut datagrams = Vec::new();

    for (line_number, line) in reader.lines().enumerate() {
        let line = line.map_err(|error| Gdl90Error::Io {
            context: "read datagram file",
            details: error.to_string(),
        })?;
        if let Some(datagram) =
            parse_datagram_line(&line).map_err(|error| Gdl90Error::InvalidField {
                field: "datagram file line",
                details: format!("line {}: {error}", line_number + 1),
            })?
        {
            datagrams.push(datagram);
        }
    }

    Ok(datagrams)
}

pub fn write_datagram_file(path: impl AsRef<Path>, datagrams: &[RecordedDatagram]) -> Result<()> {
    let file = File::create(path.as_ref()).map_err(|error| Gdl90Error::Io {
        context: "create datagram file",
        details: error.to_string(),
    })?;
    let mut writer = BufWriter::new(file);
    for datagram in datagrams {
        writeln!(writer, "{}", datagram.to_line()).map_err(|error| Gdl90Error::Io {
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
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path.as_ref())
        .map_err(|error| Gdl90Error::Io {
            context: "open datagram file for append",
            details: error.to_string(),
        })?;
    let mut writer = BufWriter::new(file);
    writeln!(writer, "{}", datagram.to_line()).map_err(|error| Gdl90Error::Io {
        context: "append datagram file",
        details: error.to_string(),
    })?;
    writer.flush().map_err(|error| Gdl90Error::Io {
        context: "flush datagram append",
        details: error.to_string(),
    })
}

pub fn parse_datagram_line(line: &str) -> std::result::Result<Option<RecordedDatagram>, String> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return Ok(None);
    }

    let (delay_ms, hex) = if let Some(rest) = trimmed.strip_prefix('@') {
        let mut parts = rest.splitn(2, char::is_whitespace);
        let delay_text = parts
            .next()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "missing delay value".to_string())?;
        let hex = parts
            .next()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "missing hex payload after delay".to_string())?;
        let delay_ms = delay_text
            .parse::<u64>()
            .map_err(|error| format!("invalid delay: {error}"))?;
        (Some(delay_ms), hex)
    } else {
        (None, trimmed)
    };

    let bytes = decode_hex(hex)?;
    Ok(Some(RecordedDatagram { delay_ms, bytes }))
}

pub fn decode_hex(input: &str) -> std::result::Result<Vec<u8>, String> {
    let filtered = input
        .chars()
        .filter(|ch| !ch.is_ascii_whitespace() && *ch != ':' && *ch != '-')
        .collect::<String>();
    if filtered.is_empty() {
        return Err("hex input is empty".to_string());
    }
    if filtered.len() % 2 != 0 {
        return Err("hex input must contain an even number of digits".to_string());
    }

    let mut out = Vec::with_capacity(filtered.len() / 2);
    let bytes = filtered.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        let pair =
            std::str::from_utf8(&bytes[index..index + 2]).map_err(|error| error.to_string())?;
        let value = u8::from_str_radix(pair, 16).map_err(|error| error.to_string())?;
        out.push(value);
        index += 2;
    }
    Ok(out)
}

pub fn encode_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(&mut out, "{byte:02X}");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_and_delayed_lines() {
        let plain = parse_datagram_line("7E 00 01 7E").unwrap().unwrap();
        assert_eq!(plain.delay_ms, None);
        assert_eq!(plain.bytes, vec![0x7E, 0x00, 0x01, 0x7E]);

        let delayed = parse_datagram_line("@250 7E00017E").unwrap().unwrap();
        assert_eq!(delayed.delay_ms, Some(250));
        assert_eq!(delayed.bytes, vec![0x7E, 0x00, 0x01, 0x7E]);
    }

    #[test]
    fn ignores_comments_and_blank_lines() {
        assert_eq!(parse_datagram_line("# comment").unwrap(), None);
        assert_eq!(parse_datagram_line("   ").unwrap(), None);
    }

    #[test]
    fn hex_encode_round_trips() {
        let bytes = vec![0x7E, 0x7D, 0x20, 0x00];
        let encoded = encode_hex(&bytes);
        assert_eq!(decode_hex(&encoded).unwrap(), bytes);
    }
}
