# gdl90

Rust library for the GDL90 binary protocol, ForeFlight GDL90 extension messages, and the Garmin GDL90 control-panel ASCII messages.

The implementation was built from the two documents provided in `/Users/lujuncheng/Downloads/gdl90`:

- `GDL90_Public_ICD_RevA.txt`
- `GDL 90_Extended_Specification.txt`

## Scope

This crate supports:

- Async HDLC framing with GDL90 CRC-CCITT FCS and byte stuffing
- Bandwidth-management scheduling for the section 2.3 output order and byte budget model
- Standard GDL90 messages from section 3 of the Garmin ICD
  - Heartbeat
  - Initialization
  - Uplink Data
  - Ownship Report
  - Traffic Report
  - Basic Report
  - Long Report
  - Height Above Terrain
  - Ownship Geometric Altitude
  - typed Basic/Long pass-through payload segmentation
    - 4-byte UAT ADS-B header
    - 13-byte State Vector
    - 12-byte Mode Status for long payloads
    - 5-byte Auxiliary State Vector for long payloads
- ForeFlight extension messages
  - ID message (`0x65/0x00`)
  - AHRS message (`0x65/0x01`)
  - supported-message subset validation for ForeFlight UDP datagrams
  - connectivity-message enforcement (`Heartbeat` or `Ownship Report`)
  - packet-size guard for the documented sub-1500-byte MTU recommendation
  - discovery interval and AHRS cadence constants from the extension doc
- UDP transport helpers
  - send framed GDL90 datagrams
  - receive and decode UDP datagrams
  - discover ForeFlight targets from the port 63093 announcement
  - derive unicast GDL90 targets from ForeFlight discovery broadcasts
  - send only ForeFlight-valid UDP datagrams through a dedicated sender
- Session tooling
  - read and write recorded UDP datagram files
  - decode saved sessions into messages
  - capture live UDP traffic to fixtures
  - replay fixtures to a target
- Analysis tooling
  - summarize recorded sessions
  - count decoded message types
  - validate saved datagrams and report malformed entries
- Report/export tooling
  - generate detailed per-datagram and per-frame reports
  - export session reports as JSON
- Uplink payload parsing for the structures documented in sections 4 and 5
  - UAT uplink payload container
  - Information Frames
  - validated minimal APDU headers
  - explicit rejection of unsupported APDU descriptor options and segmentation
  - typed FIS-B product routing
  - Generic textual DLAC APDUs
  - METAR/TAF record composition validation, including `NIL=` handling
  - NEXRAD run-length block payloads
  - Garmin sample application data field decoding for text and NEXRAD examples
- Control-panel ASCII messages from section 6
  - Call Sign
  - Mode
  - VFR Code

## Important limits from the supplied documentation

The Garmin ICD explicitly defers some nested payload details to RTCA documents and the FAA FIS-B product registry. This crate keeps those areas usable without inventing undocumented layouts:

- The 8-byte UAT-specific header is preserved as typed raw bytes because the provided Garmin text references DO-282 for its internal bit layout.
- Basic and Long ADS-B pass-through payloads are structurally segmented into their documented header/state/mode/aux sections, but the bit-level decoding of those inner fields still requires DO-282.
- NEXRAD block reference internals are preserved as the raw 3-byte indicator because the Garmin text does not reproduce the full Appendix D bit definition.
- NEXRAD payload shapes from the Garmin sample data that are not fully specified in the document are preserved raw rather than guessed.

Everything else above is fully encoded and decoded from the provided specs.

## Project layout

```text
src/
  lib.rs          Public exports
  error.rs        Shared error type
  frame.rs        CRC, byte stuffing, frame encoder/decoder, stream decoder
  bandwidth.rs    Section 2.3 bandwidth-budget scheduling and prioritization
  message.rs      Standard GDL90 message models and binary encode/decode
  report.rs       Detailed text and JSON reporting for recorded sessions
  session.rs      Recorded datagram files, hex parsing, and replay helpers
  analysis.rs     Session summary and validation helpers
  support.rs      Section coverage matrix and physical interface profiles
  transport.rs    UDP send/receive helpers, ForeFlight discovery, and validated ForeFlight sending
  uplink.rs       UAT uplink payloads, I-frames, APDUs, DLAC text, NEXRAD blocks
  foreflight.rs   ForeFlight extension messages
  control.rs      Garmin control-panel ASCII messages
  util.rs         Internal shared codecs and helpers
  bin/gdl90.rs    CLI utility
examples/
  end_to_end.rs   Framed binary message round trip
  foreflight.rs   ForeFlight device and AHRS examples
tests/
  analysis.rs     Integration coverage for session analysis and validation
  protocol.rs     Integration coverage for standard, ForeFlight, uplink, framing, and control paths
  report.rs       Integration coverage for text and JSON session reports
  session.rs      Integration coverage for recorded session files
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

Run the examples with:

```bash
cargo run --example end_to_end
cargo run --example foreflight
```

## CLI

The crate now includes a `gdl90` CLI:

```bash
cargo run --bin gdl90 -- decode-frame 7E008141DBD00802B38B7E
cargo run --bin gdl90 -- decode-stream 7E008141DBD00802B38B7E7E0B00C88008787E
cargo run --bin gdl90 -- decode-file tests/data/demo_session.txt
cargo run --bin gdl90 -- report-file tests/data/demo_session.txt
cargo run --bin gdl90 -- report-file-json tests/data/demo_session.txt report.json
cargo run --bin gdl90 -- support-status --missing
cargo run --bin gdl90 -- interface-profiles
cargo run --bin gdl90 -- analyze-file tests/data/demo_session.txt
cargo run --bin gdl90 -- validate-file tests/data/demo_session.txt
cargo run --bin gdl90 -- discover
cargo run --bin gdl90 -- listen 0.0.0.0:4000
cargo run --bin gdl90 -- capture 0.0.0.0:4000 session.txt 100
cargo run --bin gdl90 -- send-demo 192.168.1.50:4000 10 1000
cargo run --bin gdl90 -- replay-file tests/data/demo_session.txt 192.168.1.50:4000 250
```

Commands:

- `decode-frame`: decode one framed GDL90 message from hex
- `decode-stream`: decode one or more back-to-back framed messages from hex
- `decode-file`: decode every recorded datagram in a session file
- `report-file`: print a detailed per-datagram/per-frame text report
- `report-file-json`: export the same report structure as JSON
- `support-status`: print the built-in section coverage matrix
- `interface-profiles`: print RS-422 and control-panel serial profiles from the spec
- `analyze-file`: print a session summary and per-message counts
- `validate-file`: fail if any recorded datagram cannot be decoded cleanly
- `discover`: wait for a ForeFlight UDP discovery broadcast
- `listen`: listen for UDP GDL90 traffic and print decoded messages
- `capture`: record live UDP datagrams into a session file
- `send-demo`: send a recurring ForeFlight-valid demo heartbeat/ownship/geo-alt/ID/AHRS set to a target
- `replay-file`: replay a recorded session file to a UDP target

## Session File Format

Session files are plain text with one UDP datagram per line:

```text
# comment
7E008141DBD00802B38B7E
@250 7E008141DBD00802B38B7E
```

- Blank lines and lines starting with `#` are ignored.
- Hex separators such as spaces, `:` and `-` are accepted.
- `@<ms> <hex>` adds an optional replay delay before that datagram.
- Each line stores a full UDP datagram, which may contain one or more framed GDL90 messages.

## Validation

```bash
cargo fmt --check
cargo test
```

## Remaining gaps from the available material

- `3.6`: bit-level decoding of State Vector, Mode Status, and Auxiliary State Vector fields still needs the RTCA `DO-282` formulas.
- `4.1.1`: the 8-byte UAT-specific uplink header bit layout is still deferred by the Garmin ICD to `DO-282`.
- `4.3`: minimal independent APDUs are implemented and validated, but optional descriptor fields and segmented APDUs still need the external RTCA definitions.
- `4.4` and `4.5`: additional FIS-B products still require the FAA product registry definitions.
- `5.1`: exact NEXRAD geo block-reference semantics remain external-spec dependent.
- `5.2`: exact exhaustive DLAC Appendix K coverage is still not guaranteed without the referenced RTCA appendix.
