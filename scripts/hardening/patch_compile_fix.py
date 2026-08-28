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
    "scripts/hardening/patch_docs.py",
    '''version = "0.1.0"\\nedition = "2024"\\nrust-version = "1.85"\\ndescription =''',
    '''version = "0.1.0"\\nedition = "2024"\\ndescription =''',
)
