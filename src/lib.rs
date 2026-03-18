//! GDL90 binary protocol and ForeFlight extension support.
//!
//! The crate provides:
//! - Async HDLC framing with CRC-CCITT FCS and byte stuffing.
//! - Standard GDL90 message encode/decode support.
//! - ForeFlight extension message encode/decode support.
//! - Uplink payload parsing for documented I-Frames and APDUs.
//! - Control-panel ASCII message encode/decode support.

pub mod analysis;
pub mod bandwidth;
pub mod control;
mod error;
pub mod foreflight;
pub mod frame;
pub mod message;
pub mod report;
pub mod session;
pub mod transport;
pub mod uplink;
mod util;

pub use crate::error::{Gdl90Error, Result};
pub use crate::message::{
    AddressType, FrameMessageDecoder, Heartbeat, HeartbeatStatus, HeightAboveTerrain,
    Initialization, Message, OwnshipGeometricAltitude, PassThroughReport, TargetAlertStatus,
    TargetMisc, TargetReport, TrackType, VerticalFigureOfMerit,
};
