#![no_main]

use gdl90::uplink::{ApduPayload, UatUplinkPayload};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = ApduPayload::decode(data);
    if data.len() == 432 {
        if let Ok(payload) = UatUplinkPayload::decode(data) {
            let _ = payload.decoded_header();
            let _ = payload.information_frames();
            let _ = payload.apdu_payloads();
        }
    }
});
