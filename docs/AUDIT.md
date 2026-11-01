# Production-readiness audit — 7 September 2026

## Scope and provenance

Audited repository: `6639835/gdl90`, `main` at `942a06a375678f06679d0bba5dc843fb4156c95d`. The project is a Rust library plus synchronous diagnostic CLI, with binary/ASCII codecs, UDP transport, bounded reassembly, capture/replay and report generation. Database queries, web/API authentication, cloud staging/production environments and server deployment infrastructure are not present and are not invented as audit requirements.

The candidate incorporates the existing, unmerged hardening PR #3 at `adacbbba29e6d6ce4e1ea711a51218b1b872e5ea`. Those changes were independently rebuilt and tested, not accepted from the prior report alone. Original main passed 87 tests; PR #3 passed 113. Nine additional regression tests were run before the new fixes: all nine failed, then passed after repair. The final local suite contains 138 tests, passing in debug and release. CI outcomes for the exact published candidate are recorded separately in the PR/checks; configured coverage is not a substitute for executed checks.

## Findings and implemented changes

| Area | Finding | Remediation and regression evidence |
| --- | --- | --- |
| Untrusted binary input | Streaming accumulation on main was unbounded; packet boundaries were confused with stream state. | Retain PR #3's bounded decoder and source-independent UDP handling; extend bounds to direct frame decoding. Strict packet validation now reports unframed prefix/suffix bytes and raw interior flags. Explicit larger-frame opt-in remains available. |
| Arithmetic correctness | Extreme pressure altitude overflowed before validation. UAT ground track multiplication overflowed `u16`; masked ground-speed sentinels underflowed. Debug panics could become silent incorrect values in optimized builds. | Validate altitude before addition, widen the track intermediate, and check the masked speed value before subtraction. Boundary regressions are in `tests/hardening.rs`. |
| Typed APIs | Direct decoders accepted the wrong message ID; typed Basic/Long encoders could emit the wrong subtype. | Validate IDs and payload subtypes in both public directions; remove duplicated checks at the higher-level dispatcher. |
| Weather models | Publicly constructed zero-length RLE runs could underflow; fields could be silently truncated; contradictory NEXRAD variants and empty information frames could be encoded. | Make affected NEXRAD encode/decode APIs fallible, validate sizes/runs/reference bits, preserve unsupported payloads as opaque, and reject zero-length data frames. Typed text records validate structural separators and field consistency. |
| Reassembly | A malformed final TWGO segment could poison state. Refreshable age allowed duplicates to pin products. Completion cloned all stored segments. | Validate repeated headers before mutation; expire from first acceptance; assemble using borrowed segments and release accounting on completion. Test invalid-last/valid-retry, duplicate expiry and bounded accounting. |
| Filesystem and recovery | Session replacement could truncate an existing file before a later failure; capture reopened the file for every packet and could append onto a torn tail. | Shared private atomic-publication helper; owner-only new Unix files; persistent bounded append writer; torn-tail rejection; poisoned writer on failure; explicit sync checkpoints. Fault injection confirms old output survives failed replacement. |
| Transport | Extreme receive-size configuration could cause allocation failure; IPv6 discovery lost scope; socket/target address families could mismatch; OS error categories were discarded. | Clamp receive limits, retain IPv6 scope/flow information, resolve a compatible nonzero-port target, detect Windows UDP truncation, preserve `ErrorKind`, and enforce send budgets before large opaque copies. |
| CLI reliability | Malformed datagrams could stop capture; extra/unknown arguments were accepted; broken pipes panicked; demo arithmetic failed after long runs; replay could sleep on pathological delays. | Validate arguments before side effects, skip/rate-limit reporting of discarded empty/oversized datagrams, handle closed stdout normally, bound synthetic state, record capture delays, and preflight delay policy. Localhost subprocess tests exercise actual CLI recovery. |
| Reporting | Aggregate delays could wrap/panic; reports decoded the same traffic three times; network-controlled names were unescaped in text. | Saturating totals with an explicit flag, one shared decode pass, streaming JSON-to-file serialization, escaped summaries and shared validation statistics. |
| Hot paths | CRC recalculated an eight-step polynomial table entry per byte; hex encoding used per-byte formatting. | Compile-time lookup table preserving Garmin's unusual recurrence, independent reference-CRC regression, in-place frame unescaping, reused stream buffer, nibble-table hex encoding and one-pass hex parsing. |
| Configuration | Inconsistent AHRS rate/interval and non-finite/negative scheduling distances were accepted. | Validate cadence consistency and finite nonnegative distances; validate uplink slot range. |
| Supply chain | Main had no enforced CI/MSRV/security workflow; prior candidate used moving action refs. | Pin GitHub-owned actions, retain read-only credentials in verification jobs, add debug/release cross-platform checks, locked builds, packaging/rustdoc checks, dependency alerts and bounded fuzz smoke/scheduled runs. |

This is a targeted refactor, not a line-count rewrite. Existing codec modules and public protocol concepts remain intact. Filesystem publication is separated from session formatting, packet framing is shared by live/capture/report paths, and report statistics no longer trigger duplicate parsing. No database, asynchronous runtime, logging framework or additional production dependency was added.

## Dependencies and security review

The two direct runtime dependencies remain `serde` and `serde_json`. Compatible resolution updated them to 1.0.229 and 1.0.151, with their transitive dependencies, and updated the fuzz lockfile. `syn` changed to 3.0.5 through serde's proc-macro dependency; the crate does not call it directly. Rust 1.88 compilation and all tests were rerun after adopting the new lockfiles.

Fresh cargo-audit 0.22.2 results: **zero known advisories and zero warnings** for the original root lockfile, updated root lockfile and updated fuzz lockfile, using RustSec database revision `5a0ebedfe8bdd2e295b171f4162f8c977bcad9a5`. This is a dated known-advisory check, not proof that dependencies contain no vulnerabilities. Cargo metadata/lockfile evidence is retained with the audit deliverables. Review transitive license obligations when distributing binaries.

No application authentication, authorization, secrets store, SQL execution or shell-command construction from network input exists. The significant security boundaries are parser resource limits, integer arithmetic, filesystem writes, unauthenticated UDP/discovery, sensitive capture retention and CI credentials. GDL90 CRC is not an authentication mechanism. No repository-history-wide secret-scanning claim is made.

## Measured performance

Identical `benches/protocol.rs` harness, Rust 1.88.0, optimized builds, Linux x86-64 on a virtualized Intel Xeon Platinum 8573C. Each result is the median of five timed samples with warmup. Frame workloads use a deterministic 436-byte buffer; the report workload builds a report for 1,000 small datagrams. These measurements used the original dependency set on both sides to isolate code changes. They are not packet-loss, maximum throughput, latency-SLO or peak-memory measurements.

| Operation | Original main | Hardened algorithm | Speedup |
| --- | ---: | ---: | ---: |
| CRC, 436 bytes | 3.916 µs | 1.492 µs | 2.62× |
| Frame encode, 436 bytes | 4.920 µs | 2.130 µs | 2.31× |
| Frame decode, 436 bytes | 4.392 µs | 2.002 µs | 2.19× |
| Hex encode, 436 bytes | 7.549 µs | 0.982 µs | 7.69× |
| Report, 1,000 datagrams | 1.098 ms | 0.456 ms | 2.41× |

Structural memory improvements remove an intermediate frame copy, repeated stream-buffer replacement, a filtered copy of hex input, whole-session line-string accumulation during writes, duplicate report decoding and cloned reassembly segments. No percentage reduction in peak RSS is claimed. Reports remain an in-memory API and callers must set practical budgets.

## Validation and remaining release gates

Local executed checks: all 138 tests in debug and release; formatting; Clippy with warnings denied; documentation build with warnings denied; doc-test discovery (zero existing doctests); release/all-target build; verified Cargo package; compilation of both fuzz targets. Original-main and prior-candidate test/fmt/Clippy evidence was also retained. CI additionally exercises current stable Rust on Linux/macOS/Windows and sanitizer-enabled fuzzing; see the exact candidate checks for executed outcomes.

No field certification or universal production-readiness guarantee follows from these checks. Before operational deployment, require review/merge, protected-main checks, representative real-device/ForeFlight interoperability, full externally licensed RTCA/FAA schemas for any claimed coverage, overload and restart tests, and a workload-specific long-duration soak. Filesystem guarantees depend on the OS/filesystem and trusted directory. Capture rotation/backup, network allowlists, metrics, source identity, signal-aware shutdown and output-memory budgets remain embedding-application responsibilities. These are explicit limitations, not features silently presumed implemented.

Verdict: original main is **not ready for unrestricted production use**. The hardened candidate is materially stronger for the documented non-certified codec/diagnostic scope, but operational acceptance is conditional on passing the exact-revision checks and the integration gates above. It is not a certified safety-of-flight system.

## Primary references

- Garmin GDL 90 Public Interface Control Document, Rev A (FAA-hosted): https://www.faa.gov/sites/faa.gov/files/air_traffic/technology/adsb/archival/GDL90_Public_ICD_RevA.PDF
- ForeFlight extended specification: https://www.foreflight.com/connect/spec/
- Cargo manifest/MSRV and package verification: https://doc.rust-lang.org/cargo/reference/manifest.html and https://doc.rust-lang.org/cargo/commands/cargo-package.html
- RustSec advisory tooling: https://rustsec.org/
- GitHub action security guidance: https://docs.github.com/en/code-security/tutorials/secure-your-organization/protect-against-threats
- Rust Fuzz Book: https://rust-fuzz.github.io/book/cargo-fuzz/ci.html
