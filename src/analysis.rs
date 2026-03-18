use std::collections::BTreeMap;

use crate::message::Message;
use crate::session::RecordedDatagram;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionAnalysis {
    pub datagram_count: usize,
    pub total_bytes: usize,
    pub delayed_datagram_count: usize,
    pub total_declared_delay_ms: u64,
    pub decoded_message_count: usize,
    pub decode_error_count: usize,
    pub empty_datagram_count: usize,
    pub max_messages_per_datagram: usize,
    pub message_counts: BTreeMap<String, usize>,
}

impl SessionAnalysis {
    pub fn is_clean(&self) -> bool {
        self.decode_error_count == 0 && self.empty_datagram_count == 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatagramIssue {
    pub datagram_index: usize,
    pub details: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionValidation {
    pub datagram_count: usize,
    pub valid_datagram_count: usize,
    pub invalid_datagram_count: usize,
    pub issues: Vec<DatagramIssue>,
}

impl SessionValidation {
    pub fn is_valid(&self) -> bool {
        self.invalid_datagram_count == 0
    }
}

pub fn analyze_datagrams(datagrams: &[RecordedDatagram]) -> SessionAnalysis {
    let mut analysis = SessionAnalysis {
        datagram_count: datagrams.len(),
        total_bytes: 0,
        delayed_datagram_count: 0,
        total_declared_delay_ms: 0,
        decoded_message_count: 0,
        decode_error_count: 0,
        empty_datagram_count: 0,
        max_messages_per_datagram: 0,
        message_counts: BTreeMap::new(),
    };

    for datagram in datagrams {
        analysis.total_bytes += datagram.bytes.len();
        if let Some(delay_ms) = datagram.delay_ms {
            analysis.delayed_datagram_count += 1;
            analysis.total_declared_delay_ms += delay_ms;
        }

        let decoded = datagram.decode_messages();
        analysis.max_messages_per_datagram = analysis.max_messages_per_datagram.max(decoded.len());
        if decoded.is_empty() {
            analysis.empty_datagram_count += 1;
        }

        for result in decoded {
            match result {
                Ok(message) => {
                    analysis.decoded_message_count += 1;
                    *analysis
                        .message_counts
                        .entry(message.kind_name())
                        .or_default() += 1;
                }
                Err(_) => analysis.decode_error_count += 1,
            }
        }
    }

    analysis
}

pub fn validate_datagrams(datagrams: &[RecordedDatagram]) -> SessionValidation {
    let mut issues = Vec::new();
    let mut valid_datagram_count = 0usize;

    for (index, datagram) in datagrams.iter().enumerate() {
        let decoded = datagram.decode_messages();
        if decoded.is_empty() {
            issues.push(DatagramIssue {
                datagram_index: index + 1,
                details: "contains no complete framed messages".to_string(),
            });
            continue;
        }

        let mut had_issue = false;
        for result in decoded {
            if let Err(error) = result {
                had_issue = true;
                issues.push(DatagramIssue {
                    datagram_index: index + 1,
                    details: error.to_string(),
                });
            }
        }

        if !had_issue {
            valid_datagram_count += 1;
        }
    }

    SessionValidation {
        datagram_count: datagrams.len(),
        valid_datagram_count,
        invalid_datagram_count: datagrams.len().saturating_sub(valid_datagram_count),
        issues,
    }
}

trait MessageKindName {
    fn kind_name(self) -> String;
}

impl MessageKindName for Message {
    fn kind_name(self) -> String {
        match self {
            Message::Heartbeat(_) => "Heartbeat".to_string(),
            Message::Initialization(_) => "Initialization".to_string(),
            Message::UplinkData(_) => "UplinkData".to_string(),
            Message::HeightAboveTerrain(_) => "HeightAboveTerrain".to_string(),
            Message::OwnshipReport(_) => "OwnshipReport".to_string(),
            Message::OwnshipGeometricAltitude(_) => "OwnshipGeometricAltitude".to_string(),
            Message::TrafficReport(_) => "TrafficReport".to_string(),
            Message::BasicReport(_) => "BasicReport".to_string(),
            Message::LongReport(_) => "LongReport".to_string(),
            Message::ForeFlightId(_) => "ForeFlightId".to_string(),
            Message::ForeFlightAhrs(_) => "ForeFlightAhrs".to_string(),
            Message::Unknown { message_id, .. } => format!("Unknown({message_id:#04x})"),
        }
    }
}
