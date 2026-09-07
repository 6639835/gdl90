use gdl90::frame::{FrameDecoder, decode_frame};
use gdl90::message::{FrameMessageDecoder, Message};
use gdl90::uplink::{ApduPayload, UatUplinkPayload};

fn next(seed: &mut u64) -> u8 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 7;
    *seed ^= *seed << 17;
    *seed as u8
}

#[test]
fn deterministic_malformed_corpus_never_panics_or_grows_without_bound() {
    let mut seed = 0xC0FF_EE12_3456_789Au64;
    for length in 0..=1_024usize {
        let mut bytes = vec![0u8; length];
        for byte in &mut bytes {
            *byte = next(&mut seed);
        }

        let _ = decode_frame(&bytes);
        if let Ok(message) = Message::decode(&bytes) {
            let _ = message.summary();
            let _ = message.encode();
        }
        let _ = ApduPayload::decode(&bytes);
        if bytes.len() == 432 {
            let _ = UatUplinkPayload::decode(&bytes);
        }

        let mut frame_decoder = FrameDecoder::with_max_stuffed_frame_len(512).unwrap();
        let _ = frame_decoder.push(&bytes);
        let _ = frame_decoder.finish();

        let mut message_decoder = FrameMessageDecoder::with_max_stuffed_frame_len(512).unwrap();
        let _ = message_decoder.push(&bytes);
        let _ = message_decoder.finish();
    }
}
