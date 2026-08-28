# Conformance scope

## Normative sources

The implementation is reviewed against:

- Garmin, **GDL 90 Data Interface Specification, Public ICD, Rev A**: <https://www.faa.gov/sites/faa.gov/files/air_traffic/technology/adsb/archival/GDL90_Public_ICD_RevA.PDF>
- ForeFlight, **GDL 90 Extended Specification**: <https://www.foreflight.com/connect/spec/>

Garmin Rev A delegates portions of Basic/Long UAT payloads and FIS-B product encoding to RTCA/DO-282, RTCA/DO-267A, and the FAA FIS-B Product Registry. Those externally controlled requirements are not reconstructed or guessed here.

## Implemented guarantees

- Fixed-length Garmin outer messages validate length, reserved fields, ranges, and documented sentinels.
- Basic and Long pass-through messages validate their required inner UAT payload type before entering the typed `Message` model.
- Streaming frame and UDP input paths have explicit memory limits and recover after malformed input.
- Each UDP datagram is decoded independently; bytes from separate datagrams or sources are never combined.
- Unused UAT Application Data must be zero-filled.
- APDU reassembly is bounded by file count, byte count, segment count, age, source identity, and APDU generation time.
- Optional Product Descriptor forms are preserved losslessly instead of being rejected or guessed.
- ForeFlight payload budgets account for minimum IPv4/IPv6 and UDP headers so the complete packet remains below 1500 bytes.

## Explicit compatibility decisions

Garmin Rev A assigns `0x7FFE` to geometric VFOM greater than 32766 m. ForeFlight's published page currently shows `0x7EEE`. Strict decoding follows Garmin. `decode_foreflight_compatible` and `encode_for_foreflight` make the conflicting legacy behavior opt-in and reject numeric values that collide with the ForeFlight sentinel.

ForeFlight publishes an AHRS heading input range of -360.0 through +360.0 degrees while allocating bits 14–0 to the heading value without defining a signed representation. Encoding accepts that published API range and canonicalizes negative angles to their equivalent positive heading; decoding returns the canonical nonnegative wire value. This behavior remains marked `Partial` until representative-device interoperability confirms the interpretation.

## Evidence policy

A support-matrix entry is `Complete` only when the public normative source contains enough detail for implementation and the repository includes positive and negative tests. Externally delegated schemas are marked `BlockedByExternalSpec`; physical or lifecycle behavior outside the codec is marked `Partial`.
