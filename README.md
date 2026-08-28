# gdl90

Rust library and CLI for encoding, decoding, transporting, recording, and analyzing GDL90 traffic, including ForeFlight extension messages, Garmin control-panel ASCII messages, and documented UAT uplink payloads.

## What the crate currently includes

- HDLC-style GDL90 framing
  - byte stuffing and unstuffing
  - CRC-CCITT/FCS generation and validation
  - frame and stream decoders
- Standard GDL90 message support
  - Heartbeat
  - Initialization
  - Uplink Data
  - Ownship Report
  - Traffic Report
  - Basic Report
  - Long Report
  - Height Above Terrain
  - Ownship Geometric Altitude
  - pass-through ADS-B inner field decoding for basic and long reports
- ForeFlight support
  - ID message (`0x65/0x00`)
  - AHRS message (`0x65/0x01`)
  - discovery announcement parsing
  - UDP send/receive helpers using ForeFlight's documented ports
  - ForeFlight-compatible multi-message UDP datagram encoding
- Garmin control-panel ASCII message support
  - Call Sign
  - Mode
  - VFR Code
- UAT uplink parsing and encoding for the documented structures
  - Rev A information frames
  - APDU headers, civil-time variants, reserved-bit validation, and segmentation metadata
  - stateful product-file reassembly across uplink messages
  - out-of-order and duplicate-segment handling
  - TWGO repeated-header removal for products 8 and 11–17
  - Generic Text APDUs, whole-record packing, and exact six-bit DLAC encode/decode
  - DLAC control codes and code-28 run-length spaces
  - NEXRAD APDUs and block payloads
  - named, lossless raw preservation for product schemas outside Garmin Rev A
- Session tooling
  - read, write, and append recorded UDP datagram files
  - decode, validate, report, capture, and replay session traffic
- Analysis and reporting
  - per-session summaries
  - datagram validation with issue reporting
  - detailed text and JSON reports
- Support/status helpers
  - Garmin ICD section coverage matrix
  - RS-422 bus profile and connector mapping
  - control-panel serial profiles and connector mapping
- Bandwidth scheduling helpers
  - byte-budget calculation
  - message prioritization across heartbeat, ownship, traffic, and uplinks

## Project layout

```text
src/
  lib.rs          Public exports
  frame.rs        Framing, escaping, CRC/FCS, and stream decoding
  message.rs      Standard GDL90 messages and pass-through ADS-B payloads
  foreflight.rs   ForeFlight extension messages and datagram encoding
  control.rs      Garmin control-panel ASCII messages
  uplink.rs       UAT uplink payloads, APDUs, DLAC text, and NEXRAD blocks
  transport.rs    UDP send/receive helpers and ForeFlight discovery helpers
  session.rs      Recorded datagram parsing, writing, and replay helpers
  analysis.rs     Session summary and validation helpers
  report.rs       Detailed text and JSON reporting
  support.rs      ICD support matrix and interface profiles
  bandwidth.rs    Bandwidth-budget scheduling helpers
  util.rs         Internal shared codecs and helpers
  error.rs        Shared error type
  bin/gdl90.rs    CLI utility
examples/
  end_to_end.rs          Standard message framing round trip
  foreflight.rs          ForeFlight ID and AHRS examples
  uplink_reassembly.rs   Segmented FIS-B product-file reassembly
tests/
  protocol.rs     Protocol encode/decode coverage
  session.rs      Session file coverage
  analysis.rs     Analysis and validation coverage
  report.rs       Text and JSON report coverage
```

## Quick start

```rust
use gdl90::{Heartbeat, HeartbeatStatus, Message};
use gdl90::frame::decode_frame;

let heartbeat = Message::Heartbeat(Heartbeat {
    status: HeartbeatStatus {
        gps_position_valid: true,
        maintenance_required: false,
        ident: false,
        address_type_talkback: false,
        gps_battery_low: false,
        ratcs: false,
        uat_initialized: true,
        csa_requested: false,
        csa_not_available: false,
        utc_ok: true,
    },
    timestamp_seconds_since_midnight: 123,
    uplink_count: 2,
    basic_and_long_count: 17,
});

let frame = heartbeat.encode_frame().unwrap();
let clear = decode_frame(&frame).unwrap();
let decoded = Message::decode(&clear).unwrap();

assert_eq!(decoded, heartbeat);
```

The library is split into focused modules:

- `gdl90::frame` for raw framing and stream decoding
- `gdl90::message` for standard GDL90 messages
- `gdl90::foreflight` for ForeFlight extensions
- `gdl90::control` for Garmin control-panel ASCII messages
- `gdl90::uplink` for UAT uplink and APDU parsing
- `gdl90::transport` for UDP discovery, send, and receive helpers
- `gdl90::session`, `analysis`, and `report` for recorded datagram workflows
- `gdl90::support` and `bandwidth` for implementation status and scheduling helpers

## Examples

```bash
cargo run --example end_to_end
cargo run --example foreflight
cargo run --example uplink_reassembly
```

## CLI

Run the built-in CLI with:

```bash
cargo run --bin gdl90 -- --help
```

Current commands:

```text
decode-frame <hex-frame>
decode-stream <hex-stream>
decode-file <session-file>
report-file <session-file>
report-file-json <session-file> [output-file]
support-status [--missing]
interface-profiles
analyze-file <session-file>
validate-file <session-file>
listen [bind-addr]
discover [bind-addr] [timeout-seconds]
capture [bind-addr] <output-file> [count]
send-demo <target-host:port> [count] [interval-ms]
replay-file <session-file> <target-host:port> [default-interval-ms]
```

Examples:

```bash
cargo run --bin gdl90 -- decode-frame 7E008141DBD00802B38B7E
cargo run --bin gdl90 -- decode-stream 7E008141DBD00802B38B7E7E0B00C88008787E
cargo run --bin gdl90 -- decode-file tests/data/demo_session.txt
cargo run --bin gdl90 -- report-file tests/data/demo_session.txt
cargo run --bin gdl90 -- report-file-json tests/data/demo_session.txt report.json
cargo run --bin gdl90 -- analyze-file tests/data/demo_session.txt
cargo run --bin gdl90 -- validate-file tests/data/demo_session.txt
cargo run --bin gdl90 -- support-status --missing
cargo run --bin gdl90 -- interface-profiles
```

Defaults for network-oriented commands:

- ForeFlight discovery bind port: `63093`
- GDL90 traffic bind port: `4000`
- `capture` keeps running until interrupted when `count` is omitted or `0`
- `report-file-json` prints JSON to stdout when `output-file` is omitted

## Session file format

Session files are plain text with one UDP datagram per line:

```text
# comment
7E008141DBD00802B38B7E
@250 7E008141DBD00802B38B7E
```

- blank lines and lines starting with `#` are ignored
- spaces, `:` and `-` are accepted inside hex payloads
- `@<ms> <hex>` adds an optional replay delay before that datagram
- each line stores a full UDP datagram, which may contain one or more framed GDL90 messages

## Validation

Verified in this repository with:

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
```

## Protocol scope and production status

This crate implements a substantial and tested subset of Garmin GDL90 Rev A and the ForeFlight extension, but it does **not** claim certification, safety-of-flight suitability, or complete conformance to external RTCA and FAA product specifications.

The production hardening in this repository includes:

- bounded streaming frames, UDP datagrams, session files, and APDU reassembly state
- independent decoding for each UDP datagram and source-safe APDU reassembly APIs
- strict Basic/Long pass-through payload-type validation with no reporting panic path
- strict Garmin VFOM behavior plus an explicit ForeFlight compatibility mode
- Garmin-required zero-fill validation for unused UAT Application Data
- lossless preservation of optional Product Descriptor APDUs that cannot be decoded from the public ICD alone
- IP-version-aware ForeFlight UDP payload limits and a deterministic 5 Hz AHRS cadence scheduler
- fallible bandwidth configuration and checked byte accounting
- CI, adversarial parser tests, and scheduled fuzzing targets

Known boundaries are intentionally reported by `support-status --missing`:

- physical RS-422/RS-232 drivers and certified installation behavior are not implemented
- complete Basic/Long UAT semantics require RTCA/DO-282
- complete APDU/Product Descriptor and FIS-B product semantics require RTCA/DO-267A and the FAA FIS-B Product Registry
- real ForeFlight interoperability still requires testing against supported iOS/iPadOS releases and representative networks

See `docs/CONFORMANCE.md`, `docs/PRODUCTION_READINESS.md`, and `SECURITY.md` before embedding this crate in operational equipment.

## License

MIT, see [LICENSE](LICENSE).
