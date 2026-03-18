# gdl90

Rust library for the GDL90 binary protocol, ForeFlight GDL90 extension messages, and the Garmin GDL90 control-panel ASCII messages.

The implementation was built from the two documents provided in `/Users/lujuncheng/Downloads/gdl90`:

- `GDL90_Public_ICD_RevA.txt`
- `GDL 90_Extended_Specification.txt`

## Scope

This crate supports:

- Async HDLC framing with GDL90 CRC-CCITT FCS and byte stuffing
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
- ForeFlight extension messages
  - ID message (`0x65/0x00`)
  - AHRS message (`0x65/0x01`)
- UDP transport helpers
  - send framed GDL90 datagrams
  - receive and decode UDP datagrams
  - discover ForeFlight targets from the port 63093 announcement
- Uplink payload parsing for the structures documented in sections 4 and 5
  - UAT uplink payload container
  - Information Frames
  - APDU headers
  - Generic textual DLAC APDUs
  - NEXRAD run-length block payloads
- Control-panel ASCII messages from section 6
  - Call Sign
  - Mode
  - VFR Code

## Important limits from the supplied documentation

The Garmin ICD explicitly defers some nested payload details to RTCA documents and the FAA FIS-B product registry. This crate keeps those areas usable without inventing undocumented layouts:

- The 8-byte UAT-specific header is preserved as typed raw bytes because the provided Garmin text references DO-282 for its internal bit layout.
- Basic and Long ADS-B pass-through payloads are preserved as fixed raw payloads because their internal format is also defined by DO-282 rather than by the supplied Garmin text.
- NEXRAD block reference internals are preserved as the raw 3-byte indicator because the Garmin text does not reproduce the full Appendix D bit definition.

Everything else above is fully encoded and decoded from the provided specs.

## Project layout

```text
src/
  lib.rs          Public exports
  error.rs        Shared error type
  frame.rs        CRC, byte stuffing, frame encoder/decoder, stream decoder
  message.rs      Standard GDL90 message models and binary encode/decode
  transport.rs    UDP send/receive helpers and ForeFlight discovery support
  uplink.rs       UAT uplink payloads, I-frames, APDUs, DLAC text, NEXRAD blocks
  foreflight.rs   ForeFlight extension messages
  control.rs      Garmin control-panel ASCII messages
  util.rs         Internal shared codecs and helpers
  bin/gdl90.rs    CLI utility
examples/
  end_to_end.rs   Framed binary message round trip
  foreflight.rs   ForeFlight device and AHRS examples
tests/
  protocol.rs     Integration coverage for standard, ForeFlight, uplink, framing, and control paths
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
cargo run --bin gdl90 -- discover
cargo run --bin gdl90 -- listen 0.0.0.0:4000
cargo run --bin gdl90 -- send-demo 192.168.1.50:4000 10 1000
```

Commands:

- `decode-frame`: decode one framed GDL90 message from hex
- `decode-stream`: decode one or more back-to-back framed messages from hex
- `discover`: wait for a ForeFlight UDP discovery broadcast
- `listen`: listen for UDP GDL90 traffic and print decoded messages
- `send-demo`: send a recurring demo heartbeat/ownship/geo-alt/ForeFlight set to a target

## Validation

```bash
cargo fmt --check
cargo test
```
