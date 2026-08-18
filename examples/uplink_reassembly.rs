use gdl90::{Apdu, ApduHeader, ApduReassembler, ApduSegmentation, ReassemblyStatus, Result};

fn segment(apdu_number: u16, payload: &[u8]) -> Apdu {
    Apdu {
        header: ApduHeader {
            application_flag: false,
            geo_flag: false,
            product_file_flag: false,
            product_id: 8,
            segmentation_flag: true,
            time_option: 0,
            month_day: None,
            hours: 12,
            minutes: 34,
            seconds: None,
            segmentation: Some(ApduSegmentation {
                product_file_id: 42,
                product_file_length: 2,
                apdu_number,
            }),
        },
        payload: payload.to_vec(),
    }
}

fn main() -> Result<()> {
    // TWGO repeats the same six-byte payload header in every segment.
    let second = segment(2, b"ABCDEFSECOND");
    let first = segment(1, b"ABCDEFFIRST-");

    let mut reassembler = ApduReassembler::new();
    assert!(matches!(
        reassembler.push(second)?,
        ReassemblyStatus::Pending { .. }
    ));

    match reassembler.push(first)? {
        ReassemblyStatus::Complete(product) => {
            assert_eq!(product.payload, b"ABCDEFFIRST-SECOND");
            println!("reassembled {} bytes", product.payload.len());
        }
        status => panic!("unexpected reassembly status: {status:?}"),
    }

    Ok(())
}
