# Migration from the audited main revision

Baseline: `942a06a375678f06679d0bba5dc843fb4156c95d`. The audit candidate incorporates the independently re-tested changes in PR #3 (`adacbbba29e6d6ce4e1ea711a51218b1b872e5ea`) and adds further correctness, I/O, testing and performance fixes. The unreleased crate remains 0.1.0; consumers must review these source/behavior changes before adopting it.

## Fallible APIs

- `BandwidthManager::new`, custom cadence constructors, `PassThroughReport::basic_payload` and `long_payload` return `Result`.
- `NexradBlock::to_payload`, `decode_bins`, `decode_rows` and `NexradBlockReference::to_raw` now return `Result`; propagate errors with `?` instead of assuming a publicly constructible model is valid.
- `UplinkCandidate` derives application-data validity from the actual Uplink Data header; it no longer trusts a caller-supplied Boolean.
- `Gdl90Error::Io` includes `kind: std::io::ErrorKind`. Match with `..` or use `io_kind()` instead of matching error strings.

## Validation and wire behavior

Typed message codecs validate their IDs and Basic/Long payload subtypes in both directions. Pressure altitude is checked before arithmetic. Invalid NEXRAD models fail instead of wrapping, truncating or panicking. A zero-length information frame is an end marker, not an encodable data frame. A control callsign cannot contain interior padding followed by more characters.

Direct frame decoding now defaults to the same 1,024-byte stuffed-body cap as streaming. Opt in explicitly for larger experimental messages. Stored/live UDP parsing is stricter than serial resynchronization: extra bytes outside complete frames are reported, not ignored. This may increase error counts for captures that were incorrectly described as clean. Unsupported message IDs in the permitted 0..=127 range remain opaque; this is not full semantic validation of unknown extensions.

The strict Garmin geometric-altitude VFOM sentinel remains the default; use the explicit ForeFlight-compatible methods only for that interoperability profile. Negative ForeFlight headings are canonicalized as documented in CONFORMANCE.md, pending representative-device verification.

Reassembly expiry is a hard lifetime from the first segment rather than a refreshable inactivity timer. Invalid TWGO repeated headers are rejected before insertion. Applications must choose stable source scopes and call expiry during idle periods.

## Files, CLI and reports

Capture now requires `capture <bind-addr> <output-file> [count]`. Unknown commands, misspelled options and extra arguments fail before side effects. Broken stdout pipes exit normally; empty/oversized UDP datagrams no longer terminate capture. Capture stores observed inter-arrival delays and retains one file handle; rotation, graceful service shutdown and concurrent writers are not provided.

Session/report replacements are atomic publication attempts; appended captures reject a torn final line. Empty datagrams cannot be represented as a session record. JSON analysis gains `total_declared_delay_saturated`; readers should tolerate additive fields. Summaries escape potentially hostile strings, so text presentation changes but underlying decoded data does not. Replay/send-demo intervals are checked against a 24-hour per-delay policy before transmission.

See PRODUCTION_READINESS.md for filesystem durability, memory budgets, network isolation, capture privacy and release requirements.
