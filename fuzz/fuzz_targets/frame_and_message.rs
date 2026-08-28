#![no_main]

use gdl90::frame::{FrameDecoder, decode_frame};
use gdl90::{FrameMessageDecoder, Message};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = decode_frame(data);
    if let Ok(message) = Message::decode(data) {
        let _ = message.summary();
        let _ = message.encode();
    }
    let mut frames = FrameDecoder::with_max_stuffed_frame_len(1_024).unwrap();
    let _ = frames.push(data);
    let _ = frames.finish();
    let mut messages = FrameMessageDecoder::with_max_stuffed_frame_len(1_024).unwrap();
    let _ = messages.push(data);
    let _ = messages.finish();
});
