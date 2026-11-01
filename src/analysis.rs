use std::collections::BTreeMap;

use serde::Serialize;

use crate::session::RecordedDatagram;
use crate::{Gdl90Error, Message};

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
pub struct SessionAnalysis {
    pub datagram_count: usize,
    pub total_bytes: usize,
    pub delayed_datagram_count: usize,
    pub total_declared_delay_ms: u64,
    /// True when the exact sum cannot be represented in u64.
    pub total_declared_delay_saturated: bool,
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

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
pub struct DatagramIssue {
    pub datagram_index: usize,
    pub details: String,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
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

impl SessionAnalysis {
    pub(crate) fn observe(
        &mut self,
        datagram: &RecordedDatagram,
        decoded: &[crate::Result<Message>],
    ) {
        self.datagram_count += 1;
        self.total_bytes += datagram.bytes.len();
        if let Some(delay) = datagram.delay_ms {
            self.delayed_datagram_count += 1;
            match self.total_declared_delay_ms.checked_add(delay) {
                Some(total) => self.total_declared_delay_ms = total,
                None => {
                    self.total_declared_delay_ms = u64::MAX;
                    self.total_declared_delay_saturated = true;
                }
            }
        }
        self.max_messages_per_datagram = self.max_messages_per_datagram.max(decoded.len());
        if decoded.is_empty() {
            self.empty_datagram_count += 1;
        }
        for result in decoded {
            match result {
                Ok(message) => {
                    self.decoded_message_count += 1;
                    *self.message_counts.entry(message.kind_name()).or_default() += 1;
                }
                Err(_) => self.decode_error_count += 1,
            }
        }
    }
}

impl SessionValidation {
    pub(crate) fn observe(&mut self, decoded: &[Result<Message, Gdl90Error>]) {
        self.datagram_count += 1;
        let mut invalid = false;
        if decoded.is_empty() {
            self.issues.push(DatagramIssue {
                datagram_index: self.datagram_count,
                details: "contains no complete framed messages".into(),
            });
            invalid = true;
        }
        for result in decoded {
            if let Err(error) = result {
                self.issues.push(DatagramIssue {
                    datagram_index: self.datagram_count,
                    details: error.to_string(),
                });
                invalid = true;
            }
        }
        if invalid {
            self.invalid_datagram_count += 1;
        } else {
            self.valid_datagram_count += 1;
        }
    }
}

pub fn analyze_datagrams(datagrams: &[RecordedDatagram]) -> SessionAnalysis {
    let mut analysis = SessionAnalysis::default();
    for datagram in datagrams {
        analysis.observe(datagram, &datagram.decode_messages());
    }
    analysis
}

/// Syntactic framing/message validation, not interoperability or certification.
pub fn validate_datagrams_syntax(datagrams: &[RecordedDatagram]) -> SessionValidation {
    let mut validation = SessionValidation::default();
    for datagram in datagrams {
        validation.observe(&datagram.decode_messages());
    }
    validation
}

/// Backward-compatible alias for syntactic validation.
pub fn validate_datagrams(datagrams: &[RecordedDatagram]) -> SessionValidation {
    validate_datagrams_syntax(datagrams)
}
