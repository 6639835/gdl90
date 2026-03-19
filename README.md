# gdl90

Rust library and CLI for working with the GDL90 protocol, ForeFlight GDL90 extension messages, and Garmin GDL90 control-panel ASCII messages.

## What this crate covers

- HDLC-style GDL90 framing with CRC-CCITT FCS validation and byte stuffing
- Standard GDL90 message encode/decode
  - Heartbeat
  - Initialization
  - Uplink Data
  - Ownship Report
  - Traffic Report
  - Basic Report
  - Long Report
  - Height Above Terrain
  - Ownship Geometric Altitude
- ForeFlight extension message encode/decode
  - ID message (`0x65/0x00`)
  - AHRS message (`0x65/0x01`)
  - ForeFlight discovery announcement parsing
  - ForeFlight-valid UDP datagram encoding and sending
- Garmin control-panel ASCII message encode/decode
  - Call Sign
  - Mode
  - VFR Code
- Session tooling for recorded UDP datagrams
  - read, write, append, replay, and decode session files
- Analysis and reporting
  - per-session summaries
  - validation of recorded datagrams
  - detailed text and JSON reports
- Support/status helpers
  - section coverage matrix
  - RS-422 and control-panel serial interface profiles
- UAT uplink parsing for the documented outer structures
  - Information Frames
  - typed APDU headers
  - Generic Text APDUs
  - NEXRAD block payloads

## Project layout

```text
src/
  lib.rs          Public exports
  frame.rs        Framing, escaping, CRC/FCS, and stream decoding
  message.rs      Standard GDL90 messages
  foreflight.rs   ForeFlight extension messages
  control.rs      Garmin control-panel ASCII messages
  uplink.rs       UAT uplink payloads, APDUs, DLAC text, and NEXRAD blocks
  transport.rs    UDP send/receive helpers and ForeFlight discovery helpers
  session.rs      Recorded datagram file parsing and replay helpers
  analysis.rs     Session summary and validation helpers
  report.rs       Detailed text and JSON reporting
  support.rs      Protocol support matrix and interface profiles
  bandwidth.rs    Bandwidth-budget scheduling helpers
  util.rs         Internal shared codecs and helpers
  error.rs        Shared error type
  bin/gdl90.rs    CLI utility
examples/
  end_to_end.rs   Standard message framing round trip
  foreflight.rs   ForeFlight ID and AHRS examples
tests/
  protocol.rs     Protocol encode/decode coverage
  session.rs      Session file coverage
  analysis.rs     Analysis and validation coverage
  report.rs       Text and JSON report coverage
```

## Quick start

```rust
use gdl90::frame::decode_frame;
use gdl90::message::{Heartbeat, Message};

let heartbeat = Message::Heartbeat(Heartbeat {
    status: gdl90::HeartbeatStatus {
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

## Examples

```bash
cargo run --example end_to_end
cargo run --example foreflight
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

Network-oriented commands default to ForeFlight's documented UDP ports when no bind address is provided:

- discovery: `63093`
- GDL90 traffic: `4000`

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
```

## Current limits

This crate implements the documented outer protocol shapes, but a few inner payload details still depend on external material not present in this repository:

- bit-level decoding of pass-through ADS-B state/mode/auxiliary payload fields still depends on RTCA `DO-282`
- the 8-byte UAT-specific uplink header bit layout is still treated as raw structured bytes for the same reason
- optional APDU product-descriptor fields and full linked-product reassembly are not implemented
- additional FIS-B product-specific decoding still depends on external FAA product definitions
- exhaustive DLAC Appendix K behavior is not guaranteed beyond the currently implemented mappings
- exact NEXRAD geographic interpretation remains external-spec dependent

The built-in support matrix is available from:

```bash
cargo run --bin gdl90 -- support-status
cargo run --bin gdl90 -- support-status --missing
```

## License

MIT, see [LICENSE](LICENSE).
