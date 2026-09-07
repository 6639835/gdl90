# Production deployment contract

This is a synchronous Rust protocol library and diagnostic CLI, not a hosted application or certified avionics product. There is no database, authentication service, cloud environment, or migration engine to configure. Production suitability is conditional on the embedding system and the supported protocol subset in [CONFORMANCE.md](CONFORMANCE.md).

## Trust boundary

GDL90 framing CRC detects corruption; it does **not** authenticate a sender or prevent replay. ForeFlight discovery JSON is also unauthenticated. Do not expose receivers to the public Internet, trust every announcement, or infer identity from `App: ForeFlight`. Use an isolated network, host firewall, explicit permitted source/interface policy, and unicast targets. The library returns each packet's source so the embedding application can enforce that policy before consuming messages. Stable station identity should be combined with transport identity when calling `push_from` for reassembly.

Bind the CLI explicitly to the required interface. Examples involving `0.0.0.0` listen on every IPv4 interface. Run without administrator/root privileges. Use synthetic `send-demo` only on a test network: it transmits fabricated aircraft state, not live navigation data.

## Input and memory policies

The default stuffed-frame body limit is 1,024 bytes. Larger experimental frames require an explicit `decode_frame_with_limit` or streaming-decoder constructor. Network receivers default to 2,048 bytes and clamp configuration to the conservative 65,507-byte UDP payload ceiling. ForeFlight senders use destination-family budgets of 1,471 bytes for IPv4 and 1,451 for IPv6, allowing minimum IP/UDP headers while remaining **below** 1,500 bytes. IP options, tunnels, extension headers and lower path MTUs need a smaller application-selected limit.

UDP datagrams are validated independently and require distinct frame delimiters. Leading/trailing unframed garbage is an error. Serial stream decoding intentionally resynchronizes across arbitrary chunks and ignores noise outside frames. Do not use the serial decoder to silently validate stored UDP captures.

Session input defaults are bounded by file bytes, line bytes, packet count and datagram size; inspect `SessionReadLimits` and `SessionWriteLimits` before changing them. A detailed in-memory report adds raw hex, decoded models, strings and issues to the input. The input cap is **not** a peak-RSS cap. Use smaller batches for low-memory processes and enforce an output budget at the integration layer. `write_json_report` avoids a second serialized-JSON string but still accepts an in-memory report.

Reassembly defaults: 64 pending products, 1 MiB encoded segment bytes, 511 segments/product, 30 seconds from the first accepted segment. Container/metadata overhead is additional. Duplicates do not extend the deadline. Invalid repeated TWGO headers do not consume quota or replace accepted segments. Expire regularly even while input is idle, track rejected packets/expired products, and clear or re-scope state on source changes.

## Capture, durability, recovery and privacy

Treat captures as sensitive: they can contain position, aircraft identity and weather/traffic history. New capture/export files use owner-only permissions on Unix. Existing appended files keep their permissions; on Windows, configure an appropriate directory ACL. Store outputs in a trusted directory, not a directory writable by an adversary. No file API provides adversarial-directory/symlink isolation.

`DatagramFileWriter` owns one append handle and a byte budget. It requires a regular file and an existing newline-terminated tail. **One writer per file** is required; concurrent writers or external truncation/modification are unsupported. Each successful append is flushed to the OS. Call `sync_all()` for a durability checkpoint; flushing alone is not a power-loss guarantee. A write failure poisons the handle: stop and recover rather than blindly retrying a partially written record.

`write_datagram_file` and `write_json_report` publish a same-directory temporary file using rename after flushing and syncing the file. Readers do not observe a partially serialized replacement on filesystems honoring atomic rename. Unix directory syncing is attempted after publication. An error at that stage means the new file may already be visible but persistence of the directory entry could not be confirmed. These guarantees do not imply network-filesystem or hardware power-failure guarantees.

The finite-count CLI capture calls `sync_all()` on successful completion. Unbounded capture relies on process termination and flushes each record; it is not a service with a signal-aware graceful shutdown protocol. An embedding service must provide its own shutdown checkpoint. After an interrupted/disk-full append, preserve the original, validate the last complete newline-delimited record, and write recovered records to a new file. Never silently truncate the only copy. Corrupt protocol packets may be intentionally retained for diagnosis; file recovery and protocol validity are separate checks.

Implement retention, rotation and disk quotas outside the crate. Back up checkpointed files under the same privacy controls, retain checksums and software version, and periodically restore and validate a sample. No automatic off-host backup or encryption is included.

## Runtime integration

Use a bounded queue between a receive loop and reporting, persistence, or UI work. Blocking stdout, slow disks, or a slow consumer can cause OS-level UDP packet loss even with a fast codec. Choose and expose a drop/backpressure policy. Size OS socket buffers and application limits for the workload, not an arbitrary throughput target. Cadence schedulers coalesce missed periods; they do not create threads or dispatch packets.

`Gdl90Error::io_kind()` preserves timeout, interruption and other OS error categories. Retry `Interrupted`; handle `WouldBlock`/`TimedOut` according to the lifecycle; do not retry disk-full or permission failures indefinitely. The CLI skips empty/oversized network datagrams and reports discard counts at powers of two to limit log flooding. Embedded applications should expose at least packet count, CRC/parse errors, oversize rejects, queue drops, reassembly bytes/files/expirations and write failures. The library intentionally does not impose a logging backend or global subscriber.

Replay prevalidates delays against a CLI policy of at most 24 hours per packet. File parsing preserves larger `u64` delays for analysis; overflow of the aggregate delay is reported as saturation, not wrapping. `validate-file` is syntactic validation, not evidence that a capture is live, authenticated, semantically complete or suitable for navigation.

## Release gates

Run formatting, Clippy with warnings denied, debug and release tests, rustdoc, the declared Rust 1.88 MSRV, current stable Rust, packaging, and fresh root/fuzz RustSec audits. CI covers Linux, macOS and Windows; fuzz smoke tests run on pull requests and longer bounded runs are scheduled. Pin action revisions and keep them updated with Dependabot. Protect `main` and require checks plus review before merging; repository protection is an owner/admin setting, not something a source patch can enforce.

Before operational acceptance, add representative real captures, exact ForeFlight/iOS versions, IPv4/IPv6 and interface-change cases, overload/disk-full/restart testing, and a workload-specific soak test with memory/loss/latency criteria. Microbenchmarks do not replace those gates. No aviation certification, complete external RTCA schema implementation, or unrestricted production-scale guarantee is asserted.
