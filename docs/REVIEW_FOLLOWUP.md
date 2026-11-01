# PR #4 correctness follow-up — 2026-11-01

Inspected head `32d1e1ef30b2ae48278b4cfb9b636071fb4f2fc8` directly.
PR #3's incorporated work is preserved; neither PR is merged by this follow-up.

## Findings and reproductions

1. **Capture: confirmed checkpoint gap, not proven physical data loss.**
   `append` flushes to the OS; only successful finite-loop exit synchronized the
   file. Unbounded capture had no filesystem checkpoint. Each accepted record
   now synchronizes before acknowledgement, with the final synchronization
   retained. A private sink seam tests append/sync ordering and propagation of
   both errors. With the old append-only behavior, that regression fails with
   `[append, append]` instead of `[append, sync, append, sync]`.
   This verifies checkpoint ordering, not power-loss guarantees. Attempted
   syscall tracing was unavailable because this environment denies ptrace.
   Synchronous checkpoints favor durability and may reduce UDP throughput.

2. **Stream CLI: confirmed wrong input budget.**
   An encoded sequence of 400 valid frames (2,800 bytes) failed on the original
   head with `decoded hex bytes ... limit of 2048`. It now decodes all 400
   frames. `decode-stream` uses an explicit 64 KiB decoded-input budget;
   per-frame bounds, datagram parsing, and `decode_hex`'s datagram default stay
   intact. Tests cover the exact stream cap, cap+1, and retained datagram cap.
   The generic hex helper itself was not defective for its datagram use.

3. **APDU: confirmed over-partitioning, with a qualified conformance conclusion.**
   Two accepted segments with one source/product/file ID but different header
   times remained pending on the original head. Tests now complete out of order
   across time-option, hour/minute, seconds, and month/day differences, preserving
   APDU 1's header and releasing all buffered state.
   The public EASA amendments cited in CONFORMANCE.md specify association by
   product/file ID or station; they do not justify exact time equality as an
   additional generation identity. This is an inference from that public
   contract, not a claim to have verified every requirement of DO-267A.
   `generation_key` is removed. APDU number indexes a segment within the file;
   using it as a file-scope generation key would also prevent reassembly.
   Existing conflicting-slot, total-length, repeated-header, source isolation,
   quota and hard-expiry protections remain. The former generation-time test
   now verifies rejection without state mutation, followed by acceptance after
   hard expiry. Ambiguous ID reuse still requires caller lifecycle policy;
   timestamps cannot safely solve that ambiguity.

## Validation and remaining gates

Local checks use Rust 1.88.0 (MSRV) and stable 1.99.1. The suite contains 142 tests.
Run debug/release all-target/all-feature tests, formatting (root and fuzz),
Clippy with warnings denied, rustdoc with warnings denied, doc tests, both fuzz
harness compile checks, package verification, and fresh root/fuzz RustSec audits.
The stable Clippy collapsible-if warning in existing TWGO validation was cleared
with equivalent control flow compatible with MSRV.

GitHub's successful checks on the old head do not validate this change. New-head
cross-platform CI, sanitizer fuzzing and dependency-security results must be
checked separately before merging. Hardware capture replay/interoperability,
IPv4/IPv6 and interface changes, overload/disk-full/restart behavior, and
workload-specific soak criteria remain the operational acceptance gates in
PRODUCTION_READINESS.md. Repository-admin required-check enforcement must also
be verified. This follow-up does not claim those gates are satisfied.
