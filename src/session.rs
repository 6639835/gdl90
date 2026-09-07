use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::Path;

use crate::error::{Gdl90Error, Result};
use crate::message::Message;
use crate::transport::DEFAULT_MAX_DATAGRAM_SIZE;

pub const DEFAULT_MAX_SESSION_FILE_BYTES: u64 = 64 * 1024 * 1024;
pub const DEFAULT_MAX_SESSION_LINE_BYTES: usize = 8 * 1024;
pub const DEFAULT_MAX_SESSION_DATAGRAMS: usize = 1_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionReadLimits {
    pub max_file_bytes: u64,
    pub max_line_bytes: usize,
    pub max_datagrams: usize,
    pub max_datagram_bytes: usize,
}

impl Default for SessionReadLimits {
    fn default() -> Self {
        Self {
            max_file_bytes: DEFAULT_MAX_SESSION_FILE_BYTES,
            max_line_bytes: DEFAULT_MAX_SESSION_LINE_BYTES,
            max_datagrams: DEFAULT_MAX_SESSION_DATAGRAMS,
            max_datagram_bytes: DEFAULT_MAX_DATAGRAM_SIZE,
        }
    }
}

impl SessionReadLimits {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedDatagram {
    pub delay_ms: Option<u64>,
    pub bytes: Vec<u8>,
}

impl RecordedDatagram {
    pub fn decode_messages(&self) -> Vec<Result<Message>> {
        crate::transport::decode_datagram(&self.bytes)
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
    read_datagram_file_with_limits(path, SessionReadLimits::default())
}

pub fn read_datagram_file_with_limits(
    path: impl AsRef<Path>,
    limits: SessionReadLimits,
) -> Result<Vec<RecordedDatagram>> {
    let limits = limits.validate()?;
    let file =
        File::open(path.as_ref()).map_err(|error| Gdl90Error::io("open datagram file", error))?;
    let metadata = file
        .metadata()
        .map_err(|error| Gdl90Error::io("read datagram file metadata", error))?;
    if metadata.len() > limits.max_file_bytes {
        return Err(Gdl90Error::ResourceLimit {
            resource: "session file bytes",
            limit: usize::try_from(limits.max_file_bytes).unwrap_or(usize::MAX),
        });
    }

    let mut reader = BufReader::new(file);
    let mut datagrams = Vec::new();
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
            .map_err(|error| Gdl90Error::io("read datagram file", error))?;
        if bytes_read == 0 {
            break;
        }
        total_bytes_read =
            total_bytes_read
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
        }
        if let Some(datagram) = parse_datagram_line_with_limit(&line, limits.max_datagram_bytes)
            .map_err(|error| Gdl90Error::InvalidField {
                field: "datagram file line",
                details: format!("line {line_number}: {error}"),
            })?
        {
            if datagrams.len() >= limits.max_datagrams {
                return Err(Gdl90Error::ResourceLimit {
                    resource: "session datagram count",
                    limit: limits.max_datagrams,
                });
            }
            datagrams.push(datagram);
        }
    }

    Ok(datagrams)
}

pub fn write_datagram_file(path: impl AsRef<Path>, datagrams: &[RecordedDatagram]) -> Result<()> {
    write_datagram_file_with_limits(path, datagrams, SessionWriteLimits::default())
}

pub fn write_datagram_file_with_limits(
    path: impl AsRef<Path>,
    datagrams: &[RecordedDatagram],
    limits: SessionWriteLimits,
) -> Result<()> {
    let limits = limits.validate()?;
    let mut total_bytes = 0u64;
    // Validate without constructing a second copy of the entire session.
    for datagram in datagrams {
        let line = bounded_datagram_line(datagram, limits.max_datagram_bytes)?;
        total_bytes = checked_output_size(total_bytes, line.len() as u64 + 1, limits)?;
    }
    crate::storage::atomic_write(path.as_ref(), |writer| {
        for datagram in datagrams {
            writeln!(writer, "{}", datagram.to_line())?;
        }
        Ok(())
    })
    .map_err(|error| Gdl90Error::io("write datagram file", error))
}

/// Single-writer append handle. Each append is flushed to the OS. Call
/// `sync_all` at durability checkpoints; a successful flush alone does not
/// guarantee power-loss durability. Concurrent writers are not supported.
#[derive(Debug)]
pub struct DatagramFileWriter {
    writer: BufWriter<File>,
    limits: SessionWriteLimits,
    written_bytes: u64,
    failed: bool,
}

impl DatagramFileWriter {
    pub fn open(path: impl AsRef<Path>, limits: SessionWriteLimits) -> Result<Self> {
        let limits = limits.validate()?;
        let mut file = crate::storage::private_options()
            .create(true)
            .read(true)
            .append(true)
            .open(path.as_ref())
            .map_err(|error| Gdl90Error::io("open datagram file for append", error))?;
        let metadata = file
            .metadata()
            .map_err(|error| Gdl90Error::io("read datagram output metadata", error))?;
        if !metadata.is_file() {
            return Err(Gdl90Error::InvalidField {
                field: "session output",
                details: "must be a regular file".into(),
            });
        }
        let written_bytes = checked_output_size(metadata.len(), 0, limits)?;
        if written_bytes != 0 {
            file.seek(SeekFrom::End(-1))
                .map_err(|error| Gdl90Error::io("seek session tail", error))?;
            let mut tail = [0];
            file.read_exact(&mut tail)
                .map_err(|error| Gdl90Error::io("read session tail", error))?;
            if tail[0] != b'\n' {
                return Err(Gdl90Error::InvalidField {
                    field: "session output tail",
                    details: "missing final newline; repair or rotate the file before appending"
                        .into(),
                });
            }
        }
        Ok(Self {
            writer: BufWriter::new(file),
            limits,
            written_bytes,
            failed: false,
        })
    }

    pub fn written_bytes(&self) -> u64 {
        self.written_bytes
    }

    pub fn append(&mut self, datagram: &RecordedDatagram) -> Result<()> {
        if self.failed {
            return Err(Gdl90Error::InvalidField {
                field: "session writer",
                details: "previous write failed; repair or rotate the output before continuing"
                    .into(),
            });
        }
        let line = bounded_datagram_line(datagram, self.limits.max_datagram_bytes)?;
        let next = checked_output_size(self.written_bytes, line.len() as u64 + 1, self.limits)?;
        // Poison on partial write: a retry could otherwise join two records.
        self.failed = true;
        writeln!(self.writer, "{line}")
            .and_then(|()| self.writer.flush())
            .map_err(|error| Gdl90Error::io("append datagram file", error))?;
        self.written_bytes = next;
        self.failed = false;
        Ok(())
    }

    pub fn sync_all(&mut self) -> Result<()> {
        self.writer
            .flush()
            .and_then(|()| self.writer.get_ref().sync_all())
            .map_err(|error| Gdl90Error::io("synchronize datagram file", error))
    }
}

fn checked_output_size(current: u64, extra: u64, limits: SessionWriteLimits) -> Result<u64> {
    current
        .checked_add(extra)
        .filter(|total| *total <= limits.max_file_bytes)
        .ok_or(Gdl90Error::ResourceLimit {
            resource: "session output bytes",
            limit: usize::try_from(limits.max_file_bytes).unwrap_or(usize::MAX),
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
    bounded_datagram_line(datagram, limits.max_datagram_bytes)?;
    DatagramFileWriter::open(path, limits)?.append(datagram)
}

fn bounded_datagram_line(datagram: &RecordedDatagram, max_datagram_bytes: usize) -> Result<String> {
    if datagram.bytes.is_empty() {
        return Err(Gdl90Error::InvalidField {
            field: "session datagram",
            details: "empty datagrams cannot be represented by this line format".into(),
        });
    }
    if datagram.bytes.len() > max_datagram_bytes {
        return Err(Gdl90Error::ResourceLimit {
            resource: "session datagram bytes",
            limit: max_datagram_bytes,
        });
    }
    Ok(datagram.to_line())
}

pub fn parse_datagram_line(line: &str) -> Result<Option<RecordedDatagram>> {
    parse_datagram_line_with_limit(line, DEFAULT_MAX_DATAGRAM_SIZE)
}

pub fn parse_datagram_line_with_limit(
    line: &str,
    max_datagram_bytes: usize,
) -> Result<Option<RecordedDatagram>> {
    if max_datagram_bytes == 0 {
        return Err(Gdl90Error::InvalidField {
            field: "maximum datagram bytes",
            details: "must be greater than zero".to_string(),
        });
    }
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return Ok(None);
    }

    let (delay_ms, hex) = if let Some(rest) = trimmed.strip_prefix('@') {
        let mut parts = rest.splitn(2, char::is_whitespace);
        let delay_text =
            parts
                .next()
                .filter(|value| !value.is_empty())
                .ok_or(Gdl90Error::InvalidField {
                    field: "datagram delay",
                    details: "missing delay value".to_string(),
                })?;
        let hex = parts
            .next()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or(Gdl90Error::InvalidField {
                field: "datagram line",
                details: "missing hex payload after delay".to_string(),
            })?;
        let delay_ms = delay_text
            .parse::<u64>()
            .map_err(|error| Gdl90Error::InvalidField {
                field: "datagram delay",
                details: format!("invalid delay: {error}"),
            })?;
        (Some(delay_ms), hex)
    } else {
        (None, trimmed)
    };

    let bytes = decode_hex_with_limit(hex, max_datagram_bytes)?;
    Ok(Some(RecordedDatagram { delay_ms, bytes }))
}

pub fn decode_hex(input: &str) -> Result<Vec<u8>> {
    decode_hex_with_limit(input, DEFAULT_MAX_DATAGRAM_SIZE)
}

pub fn decode_hex_with_limit(input: &str, max_bytes: usize) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut high = None;
    for (offset, byte) in input.bytes().enumerate() {
        if byte.is_ascii_whitespace() || matches!(byte, b':' | b'-') {
            continue;
        }
        let nibble = match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            b'A'..=b'F' => byte - b'A' + 10,
            _ => {
                return Err(Gdl90Error::InvalidField {
                    field: "hex input",
                    details: format!("invalid ASCII hex digit at byte {offset}"),
                });
            }
        };
        if out.len() >= max_bytes {
            return Err(Gdl90Error::ResourceLimit {
                resource: "decoded hex bytes",
                limit: max_bytes,
            });
        }
        if let Some(upper) = high.take() {
            out.push((upper << 4) | nibble);
        } else {
            high = Some(nibble);
        }
    }
    if high.is_some() || out.is_empty() {
        return Err(Gdl90Error::InvalidField {
            field: "hex input",
            details: "must contain a nonempty, even number of digits".into(),
        });
    }
    Ok(out)
}

pub fn encode_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789ABCDEF";
    let mut out = String::with_capacity(bytes.len().saturating_mul(2));
    for &byte in bytes {
        out.push(DIGITS[(byte >> 4) as usize] as char);
        out.push(DIGITS[(byte & 15) as usize] as char);
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

    #[test]
    fn bounded_hex_decode_rejects_oversized_datagrams() {
        assert!(matches!(
            decode_hex_with_limit("000102", 2),
            Err(Gdl90Error::ResourceLimit {
                resource: "decoded hex bytes",
                limit: 2
            })
        ));
    }
}
