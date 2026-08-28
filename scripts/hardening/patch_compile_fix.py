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
