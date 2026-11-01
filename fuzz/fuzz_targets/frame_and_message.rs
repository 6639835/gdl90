#![no_main]
use gdl90::frame::{FrameDecoder, decode_frame};
use gdl90::{BasicUatPayload, FrameMessageDecoder, LongUatPayload, Message};
use libfuzzer_sys::fuzz_target;
fuzz_target!(|data: &[u8]| {
    let _ = decode_frame(data);
    if let Ok(message) = Message::decode(data) {
        let _ = message.summary();
        let _ = message.encode_frame_with_limit(2048);
    }
    // Exercise semantic methods, not just shallow validation of outer containers.
    if let Ok(payload) = BasicUatPayload::decode(data) {
        let _ = payload.decoded_state_vector();
        let _ = payload.encode();
    }
    if let Ok(payload) = LongUatPayload::decode(data) {
        let _ = payload.decoded_state_vector();
        let _ = payload.decoded_mode_status();
        let _ = payload.decoded_auxiliary_state_vector();
        let _ = payload.encode();
    }
    let _ = gdl90::control::ControlMessage::decode(data);
    let _ = gdl90::transport::decode_datagram(data);
    if let Ok(text) = std::str::from_utf8(data) {
        let _ = gdl90::transport::ForeFlightDiscoveryAnnouncement::parse(text);
        let _ = gdl90::session::parse_datagram_line_with_limit(text, 2048);
    }
    let mut frames = FrameDecoder::new();
    let mut messages = FrameMessageDecoder::new();
    for chunk in data.chunks(17) {
        let _ = frames.push(chunk);
        let _ = messages.push(chunk);
    }
    let _ = frames.finish();
    let _ = messages.finish();
});
