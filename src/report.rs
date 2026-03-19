use serde::Serialize;

use crate::analysis::{SessionAnalysis, SessionValidation, analyze_datagrams, validate_datagrams};
use crate::frame::FrameDecoder;
use crate::message::Message;
use crate::session::{RecordedDatagram, encode_hex};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SessionReport {
    pub analysis: SessionAnalysis,
    pub validation: SessionValidation,
    pub datagrams: Vec<DatagramReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DatagramReport {
    pub index: usize,
    pub delay_ms: Option<u64>,
    pub size_bytes: usize,
    pub raw_hex: String,
    pub frame_count: usize,
    pub frames: Vec<FrameReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum FrameReport {
    Decoded {
        index: usize,
        clear_hex: String,
        message_id: u8,
        kind: String,
        summary: String,
    },
    MessageError {
        index: usize,
        clear_hex: String,
        message_id: Option<u8>,
        error: String,
    },
    FrameError {
        index: usize,
        error: String,
    },
}

pub fn build_session_report(datagrams: &[RecordedDatagram]) -> SessionReport {
    let analysis = analyze_datagrams(datagrams);
    let validation = validate_datagrams(datagrams);
    let mut reports = Vec::with_capacity(datagrams.len());

    for (datagram_index, datagram) in datagrams.iter().enumerate() {
        let mut decoder = FrameDecoder::new();
        let frame_results = decoder.push(&datagram.bytes);
        let mut frames = Vec::with_capacity(frame_results.len());

        for (frame_index, frame_result) in frame_results.into_iter().enumerate() {
            match frame_result {
                Ok(clear) => match Message::decode(&clear) {
                    Ok(message) => frames.push(FrameReport::Decoded {
                        index: frame_index + 1,
                        clear_hex: encode_hex(&clear),
                        message_id: message.message_id(),
                        kind: message.kind_name(),
                        summary: message.summary(),
                    }),
                    Err(error) => frames.push(FrameReport::MessageError {
                        index: frame_index + 1,
                        clear_hex: encode_hex(&clear),
                        message_id: clear.first().copied(),
                        error: error.to_string(),
                    }),
                },
                Err(error) => frames.push(FrameReport::FrameError {
                    index: frame_index + 1,
                    error: error.to_string(),
                }),
            }
        }

        reports.push(DatagramReport {
            index: datagram_index + 1,
            delay_ms: datagram.delay_ms,
            size_bytes: datagram.bytes.len(),
            raw_hex: encode_hex(&datagram.bytes),
            frame_count: frames.len(),
            frames,
        });
    }

    SessionReport {
        analysis,
        validation,
        datagrams: reports,
    }
}

pub fn render_text_report(report: &SessionReport) -> String {
    let mut out = String::new();

    out.push_str(&render_analysis_text(&report.analysis));
    push_line(&mut out, "datagram details:".to_string());
    for datagram in &report.datagrams {
        push_line(
            &mut out,
            format!(
                "  datagram {} delay={:?} size={} frames={}",
                datagram.index, datagram.delay_ms, datagram.size_bytes, datagram.frame_count
            ),
        );
        for frame in &datagram.frames {
            match frame {
                FrameReport::Decoded {
                    index,
                    kind,
                    summary,
                    ..
                } => {
                    push_line(
                        &mut out,
                        format!("    frame {index} kind={kind} summary={summary}"),
                    );
                }
                FrameReport::MessageError { index, error, .. }
                | FrameReport::FrameError { index, error } => {
                    push_line(&mut out, format!("    frame {index} error: {error}"));
                }
            }
        }
        if datagram.frames.is_empty() {
            push_line(&mut out, "    no complete framed messages".to_string());
        }
    }

    out
}

pub fn render_analysis_text(analysis: &SessionAnalysis) -> String {
    let mut out = String::new();

    push_line(&mut out, format!("datagrams: {}", analysis.datagram_count));
    push_line(&mut out, format!("total bytes: {}", analysis.total_bytes));
    push_line(
        &mut out,
        format!("delayed datagrams: {}", analysis.delayed_datagram_count),
    );
    push_line(
        &mut out,
        format!(
            "declared replay delay ms: {}",
            analysis.total_declared_delay_ms
        ),
    );
    push_line(
        &mut out,
        format!("decoded messages: {}", analysis.decoded_message_count),
    );
    push_line(
        &mut out,
        format!("decode errors: {}", analysis.decode_error_count),
    );
    push_line(
        &mut out,
        format!("empty datagrams: {}", analysis.empty_datagram_count),
    );
    push_line(
        &mut out,
        format!(
            "max messages per datagram: {}",
            analysis.max_messages_per_datagram
        ),
    );
    push_line(&mut out, "message counts:".to_string());
    for (kind, count) in &analysis.message_counts {
        push_line(&mut out, format!("  {kind}: {count}"));
    }

    out
}

pub fn render_validation_text(validation: &SessionValidation) -> String {
    let mut out = String::new();

    if validation.is_valid() {
        push_line(
            &mut out,
            format!(
                "valid: {} datagrams, no decode issues",
                validation.datagram_count
            ),
        );
    } else {
        push_line(
            &mut out,
            format!(
                "invalid: {} of {} datagrams have issues",
                validation.invalid_datagram_count, validation.datagram_count
            ),
        );
        for issue in &validation.issues {
            push_line(
                &mut out,
                format!("  datagram {}: {}", issue.datagram_index, issue.details),
            );
        }
    }

    out
}

pub fn render_json_report(report: &SessionReport, pretty: bool) -> serde_json::Result<String> {
    if pretty {
        serde_json::to_string_pretty(report)
    } else {
        serde_json::to_string(report)
    }
}

fn push_line(out: &mut String, line: String) {
    out.push_str(&line);
    out.push('\n');
}
