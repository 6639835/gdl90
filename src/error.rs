use core::fmt;

pub type Result<T> = std::result::Result<T, Gdl90Error>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Gdl90Error {
    InvalidLength {
        context: &'static str,
        expected: &'static str,
        actual: usize,
    },
    InvalidField {
        field: &'static str,
        details: String,
    },
    InvalidMessageId(u8),
    MissingFrameFlag,
    FrameTooShort,
    DanglingEscape,
    InvalidEscapeByte(u8),
    CrcMismatch {
        expected: u16,
        actual: u16,
    },
    Utf8 {
        field: &'static str,
    },
    UnsupportedCharacter {
        context: &'static str,
        ch: char,
    },
    ControlChecksumMismatch {
        expected: u8,
        actual: u8,
    },
    ControlFormat(&'static str),
    Io {
        context: &'static str,
        details: String,
    },
}

impl fmt::Display for Gdl90Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLength {
                context,
                expected,
                actual,
            } => write!(
                f,
                "{context} length mismatch: expected {expected}, got {actual}"
            ),
            Self::InvalidField { field, details } => write!(f, "invalid {field}: {details}"),
            Self::InvalidMessageId(id) => write!(f, "unsupported message id {id:#04x}"),
            Self::MissingFrameFlag => write!(f, "frame is missing start or end flag"),
            Self::FrameTooShort => write!(f, "frame is too short"),
            Self::DanglingEscape => write!(f, "frame ended with a dangling escape byte"),
            Self::InvalidEscapeByte(byte) => write!(f, "invalid escaped byte {byte:#04x}"),
            Self::CrcMismatch { expected, actual } => {
                write!(
                    f,
                    "crc mismatch: expected {expected:#06x}, got {actual:#06x}"
                )
            }
            Self::Utf8 { field } => write!(f, "{field} is not valid UTF-8"),
            Self::UnsupportedCharacter { context, ch } => {
                write!(f, "unsupported character {ch:?} in {context}")
            }
            Self::ControlChecksumMismatch { expected, actual } => write!(
                f,
                "control checksum mismatch: expected {expected:02X}, got {actual:02X}"
            ),
            Self::ControlFormat(details) => write!(f, "invalid control message format: {details}"),
            Self::Io { context, details } => write!(f, "{context}: {details}"),
        }
    }
}

impl std::error::Error for Gdl90Error {}
