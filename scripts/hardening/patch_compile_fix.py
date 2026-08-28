from __future__ import annotations

from common import MARKER, replace_once

if MARKER.exists():
    raise SystemExit(0)

replace_once(
    "src/uplink.rs",
    '''        let existing = self
            .pending
            .get(&scope)
            .ok_or(Gdl90Error::InvalidField {
                field: "APDU product file",
                details: "pending state disappeared unexpectedly".to_string(),
            })?;
        if existing.total != total {
            if inserted_new {
                self.remove_scope(scope);
            }
            return Err(Gdl90Error::InvalidField {
                field: "APDU product file length",
                details: format!(
                    "product file {key:?} was declared with {} segments and later with {total}",
                    existing.total
                ),
            });
        }

        let duplicate = match &existing.segments[index] {''',
    '''        let declared_total = self
            .pending
            .get(&scope)
            .ok_or(Gdl90Error::InvalidField {
                field: "APDU product file",
                details: "pending state disappeared unexpectedly".to_string(),
            })?
            .total;
        if declared_total != total {
            if inserted_new {
                self.remove_scope(scope);
            }
            return Err(Gdl90Error::InvalidField {
                field: "APDU product file length",
                details: format!(
                    "product file {key:?} was declared with {declared_total} segments and later with {total}"
                ),
            });
        }

        let existing = self
            .pending
            .get(&scope)
            .ok_or(Gdl90Error::InvalidField {
                field: "APDU product file",
                details: "pending state disappeared unexpectedly".to_string(),
            })?;
        let duplicate = match &existing.segments[index] {''',
)

replace_once(
    "src/uplink.rs",
    '''        if let Some(segments) = complete_segments {
            self.remove_scope(scope);
            return Ok(ReassemblyStatus::Complete(assemble_product_file(
                key, &segments,
            )?));
        }''',
    '''        if let Some(segments) = complete_segments {
            let complete = assemble_product_file(key, &segments)?;
            self.remove_scope(scope);
            return Ok(ReassemblyStatus::Complete(complete));
        }''',
)

replace_once(
    "src/uplink.rs",
    "#[derive(Debug, Clone)]\npub struct ApduReassembler {",
    "#[derive(Debug, Clone, Default)]\npub struct ApduReassembler {",
)

replace_once(
    "src/uplink.rs",
    '''impl Default for ApduReassembler {
    fn default() -> Self {
        Self {
            pending: BTreeMap::new(),
            limits: ReassemblyLimits::default(),
            buffered_bytes: 0,
        }
    }
}

''',
    "",
)

replace_once(
    "src/uplink.rs",
    "for chunk in bytes[item_offset..].chunks_exact(3) {",
    "for chunk in bytes[item_offset..].as_chunks::<3>().0 {",
)

replace_once(
    "src/uplink.rs",
    "for chunk in values.chunks_exact(4) {",
    "for chunk in values.as_chunks::<4>().0 {",
)

replace_once(
    "src/uplink.rs",
    '''#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ApduReassemblyScope {
    pub source_id: u64,
    pub key: ApduProductFileKey,
}''',
    '''#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ApduReassemblyScope {
    pub source_id: u64,
    pub key: ApduProductFileKey,
    /// Packed APDU generation time, separating reused product-file ids.
    pub generation_key: u32,
}

fn apdu_generation_key(header: ApduHeader) -> u32 {
    let (month, day) = header
        .month_day
        .map(|value| (value.month, value.day))
        .unwrap_or((0, 0));
    (u32::from(header.time_option) << 26)
        | (u32::from(month) << 22)
        | (u32::from(day) << 17)
        | (u32::from(header.hours) << 12)
        | (u32::from(header.minutes) << 6)
        | u32::from(header.seconds.unwrap_or(0))
}''',
)

replace_once(
    "src/uplink.rs",
    "        let scope = ApduReassemblyScope { source_id, key };",
    '''        let scope = ApduReassemblyScope {
            source_id,
            key,
            generation_key: apdu_generation_key(apdu.header),
        };''',
)

replace_once(
    "tests/protocol.rs",
    '''#[test]
fn reassembly_is_bounded_scoped_and_expiring() {''',
    '''#[test]
fn reassembly_separates_reused_ids_by_generation_time() {
    let start = Instant::now();
    let mut reassembler = ApduReassembler::new();
    let mut first_generation = segmented_apdu(413, 7, 2, 1, b"ONE");
    first_generation.header.hours = 1;
    let mut second_generation = segmented_apdu(413, 7, 2, 1, b"TWO");
    second_generation.header.hours = 2;

    reassembler
        .push_from_at(10, first_generation, start)
        .unwrap();
    reassembler
        .push_from_at(10, second_generation, start)
        .unwrap();

    let scopes = reassembler.pending_scopes();
    assert_eq!(scopes.len(), 2);
    assert_ne!(scopes[0].generation_key, scopes[1].generation_key);
}

#[test]
fn reassembly_is_bounded_scoped_and_expiring() {''',
)

replace_once(
    "scripts/hardening/patch_docs.py",
    '''version = "0.1.0"\\nedition = "2024"\\nrust-version = "1.85"\\ndescription =''',
    '''version = "0.1.0"\\nedition = "2024"\\ndescription =''',
)

replace_once(
    "scripts/hardening/patch_docs.py",
    '''pub fn decode_hex(input: &str) -> Result<Vec<u8>> {
    decode_hex_with_limit(input, usize::MAX / 2)
}''',
    '''pub fn decode_hex(input: &str) -> Result<Vec<u8>> {
    decode_hex_with_limit(input, DEFAULT_MAX_DATAGRAM_SIZE)
}''',
)

replace_once(
    "scripts/hardening/patch_docs.py",
    '''jobs:
  quality:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable''',
    '''jobs:
  quality:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
      - uses: dtolnay/rust-toolchain@stable''',
)

replace_once(
    "scripts/hardening/patch_docs.py",
    '''      - name: Clippy
        run: cargo clippy --all-targets -- -D warnings
''',
    '''      - name: Clippy
        run: cargo clippy --all-targets -- -D warnings
      - name: Compile fuzz targets
        run: cargo check --manifest-path fuzz/Cargo.toml --all-targets
''',
)

replace_once(
    "scripts/hardening/patch_docs.py",
    '''jobs:
  fuzz:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@nightly''',
    '''jobs:
  fuzz:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
      - uses: dtolnay/rust-toolchain@nightly''',
)
