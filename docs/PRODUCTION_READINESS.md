# Production readiness

## What this repository now protects

- Arbitrary network input cannot grow the streaming frame buffer without bound.
- UDP packet boundaries and sender identities are not mixed.
- Oversized UDP datagrams and session inputs fail closed with structured errors.
- Pass-through report decoding and reporting do not use `expect` on attacker-controlled bytes.
- Reassembly state has configurable count, byte, segment, age, and source limits.
- Invalid bandwidth configuration returns an error instead of dividing by zero or overflowing.
- CI requires formatting, all tests, adversarial tests, and warnings-as-errors Clippy.

## Embedding application responsibilities

The embedding application must still provide:

- monotonic scheduling and backpressure integration around the cadence and bandwidth helpers
- network-interface change handling, discovery lifecycle, logging, metrics, and shutdown behavior
- serial drivers and installation-specific electrical validation when RS-422/RS-232 is required
- reassembly source identifiers derived from a stable station or transport identity
- capture-file rotation, retention, disk quotas, and atomic export publication beyond the crate's default file-size guard
- explicit text/JSON report-output budgets appropriate for the embedding process
- interoperability testing against the exact ForeFlight release and network environment being supported
- licensed external specifications and their conformance vectors when claiming full UAT/FIS-B compliance

## Aviation limitation

This project is not FAA-approved, is not a TSO authorization, and has not undergone a DO-178C software life-cycle. It must not be represented as certified safety-of-flight software. Operational use requires an independent system safety assessment, requirements traceability, hardware-in-the-loop testing, fault injection, soak testing, and the approvals applicable to the equipment.
