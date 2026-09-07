//! Dependency-free, reproducible microbenchmarks. Run `cargo bench --bench protocol`.
//! Reports medians of five samples; not an end-to-end network capacity claim.
use gdl90::{
    frame::{crc16_ccitt, decode_frame, encode_frame},
    report::build_session_report,
    session::{RecordedDatagram, encode_hex},
};
use std::{hint::black_box, time::Instant};
fn measure(name: &str, iterations: usize, mut operation: impl FnMut()) {
    for _ in 0..100 {
        operation();
    }
    let mut samples = [0f64; 5];
    for sample in &mut samples {
        let start = Instant::now();
        for _ in 0..iterations {
            operation();
        }
        *sample = start.elapsed().as_nanos() as f64 / iterations as f64;
    }
    samples.sort_by(f64::total_cmp);
    println!("{name},{iterations},{:.2}", samples[2]);
}
fn main() {
    if !std::env::args().any(|arg| arg == "--bench") {
        return;
    }
    println!("operation,iterations,median_ns_per_operation");
    let payload: Vec<u8> = (0..436).map(|i| (i % 251) as u8).collect();
    let frame = encode_frame(&payload);
    measure("crc_436", 100_000, || {
        black_box(crc16_ccitt(black_box(&payload)));
    });
    measure("encode_frame_436", 100_000, || {
        black_box(encode_frame(black_box(&payload)));
    });
    measure("decode_frame_436", 100_000, || {
        black_box(decode_frame(black_box(&frame)).unwrap());
    });
    measure("hex_encode_436", 100_000, || {
        black_box(encode_hex(black_box(&payload)));
    });
    let records = vec![
        RecordedDatagram {
            delay_ms: Some(100),
            bytes: encode_frame(&[4, 0, 0])
        };
        1_000
    ];
    measure("report_1000_datagrams", 50, || {
        black_box(build_session_report(black_box(&records)));
    });
}
