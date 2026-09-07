#![no_main]
use gdl90::uplink::{
    Apdu, ApduHeader, ApduPayload, ApduReassembler, ApduSegmentation, NexradBlock,
    ReassemblyLimits, UatUplinkPayload,
};
use libfuzzer_sys::fuzz_target;
fuzz_target!(|data: &[u8]| {
    if let Ok(apdu) = ApduPayload::decode(data) {
        let _ = apdu.encode();
    }
    if let Ok(apdu) = Apdu::decode(data) {
        let _ = apdu.decode_product();
    }
    if let Ok(block) = NexradBlock::from_payload(data) {
        let _ = block.to_payload();
        let _ = block.decode_bins();
        let _ = block.decode_rows();
    }
    if let Ok(payload) = UatUplinkPayload::decode(data) {
        let _ = payload.decoded_header();
        let _ = payload.information_frames();
        let _ = payload.apdu_payloads();
    }
    let mut reassembler = ApduReassembler::with_limits(ReassemblyLimits {
        max_pending_files: 2,
        max_buffered_bytes: 2048,
        max_segments_per_file: 2,
        ..ReassemblyLimits::default()
    })
    .unwrap();
    // Force valid segmentation metadata so fuzzing reaches state mutation and recovery.
    for chunk in data.chunks(32) {
        let apdu = Apdu {
            header: ApduHeader {
                application_flag: false,
                geo_flag: false,
                product_file_flag: false,
                product_id: 8,
                segmentation_flag: true,
                time_option: 0,
                month_day: None,
                hours: 0,
                minutes: 0,
                seconds: None,
                segmentation: Some(ApduSegmentation {
                    product_file_id: 1,
                    product_file_length: 2,
                    apdu_number: u16::from(chunk[0] % 2) + 1,
                }),
            },
            payload: chunk.to_vec(),
        };
        let _ = reassembler.push(apdu);
        assert!(reassembler.buffered_bytes() <= 2048);
        assert!(reassembler.pending_file_count() <= 2);
    }
});
