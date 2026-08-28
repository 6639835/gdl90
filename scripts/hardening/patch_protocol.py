from __future__ import annotations

from common import MARKER, read, replace_once, replace_regex, write

if MARKER.exists():
    raise SystemExit(0)

replace_once(
    "src/message.rs",
    '''pub enum VerticalFigureOfMerit {\n    Meters(u16),\n    NotAvailable,\n    GreaterThan32766,\n}''',
    '''pub enum VerticalFigureOfMerit {\n    Meters(u16),\n    NotAvailable,\n    GreaterThan32766,\n}\n\n/// Selects the documented Garmin sentinel or the conflicting legacy value\n/// currently published by ForeFlight. Strict GDL90 decoding remains Garmin Rev A.\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum VerticalFigureOfMeritEncoding {\n    GarminRevA,\n    ForeFlightLegacy,\n}''',
)

replace_regex(
    "src/message.rs",
    r'''impl OwnshipGeometricAltitude \{.*?\n\}\n\n#\[derive\(Debug, Clone, PartialEq\)\]''',
    r'''impl OwnshipGeometricAltitude {
    pub const LEN: usize = 5;
    const GARMIN_GREATER_THAN_32766: u16 = 0x7FFE;
    const FOREFLIGHT_GREATER_THAN_32766: u16 = 0x7EEE;
    const NOT_AVAILABLE: u16 = 0x7FFF;

    /// Strict Garmin GDL90 Public ICD Rev A decoding.
    pub fn decode(payload: &[u8]) -> Result<Self> {
        Self::decode_with_foreflight_compatibility(payload, false)
    }

    /// Accepts both Garmin's 0x7FFE sentinel and ForeFlight's published 0x7EEE
    /// legacy value. The raw conflict is explicit rather than silently changing
    /// the strict GDL90 decoder.
    pub fn decode_foreflight_compatible(payload: &[u8]) -> Result<Self> {
        Self::decode_with_foreflight_compatibility(payload, true)
    }

    fn decode_with_foreflight_compatibility(
        payload: &[u8],
        accept_foreflight_sentinel: bool,
    ) -> Result<Self> {
        if payload.len() != Self::LEN {
            return Err(Gdl90Error::InvalidLength {
                context: "ownship geometric altitude message",
                expected: "5 bytes",
                actual: payload.len(),
            });
        }
        let raw_altitude = i16::from_be_bytes([payload[1], payload[2]]);
        let raw_metrics = u16::from_be_bytes([payload[3], payload[4]]);
        let raw_vfom = raw_metrics & 0x7FFF;
        Ok(Self {
            altitude_feet: i32::from(raw_altitude) * 5,
            vertical_warning: (raw_metrics & 0x8000) != 0,
            vertical_figure_of_merit: match raw_vfom {
                Self::NOT_AVAILABLE => VerticalFigureOfMerit::NotAvailable,
                Self::GARMIN_GREATER_THAN_32766 => {
                    VerticalFigureOfMerit::GreaterThan32766
                }
                Self::FOREFLIGHT_GREATER_THAN_32766 if accept_foreflight_sentinel => {
                    VerticalFigureOfMerit::GreaterThan32766
                }
                meters => VerticalFigureOfMerit::Meters(meters),
            },
        })
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        self.encode_with_vfom_encoding(VerticalFigureOfMeritEncoding::GarminRevA)
    }

    pub fn encode_for_foreflight(&self) -> Result<Vec<u8>> {
        self.encode_with_vfom_encoding(VerticalFigureOfMeritEncoding::ForeFlightLegacy)
    }

    pub fn encode_with_vfom_encoding(
        &self,
        encoding: VerticalFigureOfMeritEncoding,
    ) -> Result<Vec<u8>> {
        if self.altitude_feet % 5 != 0 {
            return Err(Gdl90Error::InvalidField {
                field: "geometric altitude",
                details: "must be a 5-foot increment".to_string(),
            });
        }
        let altitude_units = self.altitude_feet / 5;
        if !(i32::from(i16::MIN)..=i32::from(i16::MAX)).contains(&altitude_units) {
            return Err(Gdl90Error::InvalidField {
                field: "geometric altitude",
                details: "does not fit in signed 16-bit 5-foot units".to_string(),
            });
        }
        let greater_than_sentinel = match encoding {
            VerticalFigureOfMeritEncoding::GarminRevA => Self::GARMIN_GREATER_THAN_32766,
            VerticalFigureOfMeritEncoding::ForeFlightLegacy => {
                Self::FOREFLIGHT_GREATER_THAN_32766
            }
        };
        let vfom = match self.vertical_figure_of_merit {
            VerticalFigureOfMerit::Meters(value) => value.min(greater_than_sentinel),
            VerticalFigureOfMerit::NotAvailable => Self::NOT_AVAILABLE,
            VerticalFigureOfMerit::GreaterThan32766 => greater_than_sentinel,
        };

        let mut out = Vec::with_capacity(Self::LEN);
        out.push(OWNSHIP_GEOMETRIC_ALTITUDE_MESSAGE_ID);
        out.extend_from_slice(&(altitude_units as i16).to_be_bytes());
        out.extend_from_slice(
            &(((self.vertical_warning as u16) << 15) | (vfom & 0x7FFF)).to_be_bytes(),
        );
        Ok(out)
    }
}

#[derive(Debug, Clone, PartialEq)]''',
)

replace_once(
    "src/message.rs",
    '''impl PassThroughReport<18> {\n    pub fn basic_payload(&self) -> BasicUatPayload {\n        BasicUatPayload::decode(&self.payload).expect("fixed-size basic payload should decode")\n    }''',
    '''impl PassThroughReport<18> {\n    pub fn basic_payload(&self) -> Result<BasicUatPayload> {\n        BasicUatPayload::decode(&self.payload)\n    }''',
)
replace_once(
    "src/message.rs",
    '''impl PassThroughReport<34> {\n    pub fn long_payload(&self) -> LongUatPayload {\n        LongUatPayload::decode(&self.payload).expect("fixed-size long payload should decode")\n    }''',
    '''impl PassThroughReport<34> {\n    pub fn long_payload(&self) -> Result<LongUatPayload> {\n        LongUatPayload::decode(&self.payload)\n    }''',
)

replace_once(
    "src/message.rs",
    '''            Self::BasicReport(message) => {\n                let payload = message.basic_payload();\n                format!(\n                    "tor={:?} type={} qualifier={} address={:#08x}",\n                    message.time_of_reception,\n                    payload.header.payload_type_code,\n                    payload.header.address_qualifier,\n                    payload.header.address\n                )\n            }''',
    '''            Self::BasicReport(message) => match message.basic_payload() {\n                Ok(payload) => format!(\n                    "tor={:?} type={} qualifier={} address={:#08x}",\n                    message.time_of_reception,\n                    payload.header.payload_type_code,\n                    payload.header.address_qualifier,\n                    payload.header.address\n                ),\n                Err(error) => format!("invalid basic UAT payload: {error}"),\n            }''',
)
replace_once(
    "src/message.rs",
    '''            Self::LongReport(message) => {\n                let payload = message.long_payload();\n                format!(\n                    "tor={:?} type={} qualifier={} address={:#08x}",\n                    message.time_of_reception,\n                    payload.header.payload_type_code,\n                    payload.header.address_qualifier,\n                    payload.header.address\n                )\n            }''',
    '''            Self::LongReport(message) => match message.long_payload() {\n                Ok(payload) => format!(\n                    "tor={:?} type={} qualifier={} address={:#08x}",\n                    message.time_of_reception,\n                    payload.header.payload_type_code,\n                    payload.header.address_qualifier,\n                    payload.header.address\n                ),\n                Err(error) => format!("invalid long UAT payload: {error}"),\n            }''',
)

replace_once(
    "src/message.rs",
    '''            BASIC_REPORT_MESSAGE_ID => Ok(Self::BasicReport(PassThroughReport::<18>::decode(\n                "basic report",\n                payload,\n            )?)),\n            LONG_REPORT_MESSAGE_ID => Ok(Self::LongReport(PassThroughReport::<34>::decode(\n                "long report",\n                payload,\n            )?)),''',
    '''            BASIC_REPORT_MESSAGE_ID => {\n                let report = PassThroughReport::<18>::decode("basic report", payload)?;\n                report.basic_payload()?;\n                Ok(Self::BasicReport(report))\n            }\n            LONG_REPORT_MESSAGE_ID => {\n                let report = PassThroughReport::<34>::decode("long report", payload)?;\n                report.long_payload()?;\n                Ok(Self::LongReport(report))\n            }''',
)
replace_once(
    "src/message.rs",
    '''            Self::BasicReport(message) => message.encode(BASIC_REPORT_MESSAGE_ID),\n            Self::LongReport(message) => message.encode(LONG_REPORT_MESSAGE_ID),''',
    '''            Self::BasicReport(message) => {\n                message.basic_payload()?;\n                message.encode(BASIC_REPORT_MESSAGE_ID)\n            }\n            Self::LongReport(message) => {\n                message.long_payload()?;\n                message.encode(LONG_REPORT_MESSAGE_ID)\n            }''',
)

replace_once(
    "src/message.rs",
    '''    pub fn new() -> Self {\n        Self::default()\n    }\n\n    pub fn push(&mut self, bytes: &[u8]) -> Vec<Result<Message>> {''',
    '''    pub fn new() -> Self {\n        Self::default()\n    }\n\n    pub fn with_max_stuffed_frame_len(max_stuffed_frame_len: usize) -> Result<Self> {\n        Ok(Self {\n            frame_decoder: FrameDecoder::with_max_stuffed_frame_len(max_stuffed_frame_len)?,\n        })\n    }\n\n    pub fn push(&mut self, bytes: &[u8]) -> Vec<Result<Message>> {''',
)

replace_once(
    "src/uplink.rs",
    "use std::collections::BTreeMap;",
    "use std::collections::BTreeMap;\nuse std::time::{Duration, Instant};",
)
replace_once(
    "src/uplink.rs",
    '''            if length == 0 {\n                break;\n            }''',
    '''            if length == 0 {\n                break;\n            }''',
)
replace_once(
    "src/uplink.rs",
    '''            offset += total;\n        }\n\n        Ok(frames)\n    }''',
    '''            offset += total;\n        }\n\n        if self.application_data[offset..].iter().any(|byte| *byte != 0) {\n            return Err(Gdl90Error::InvalidField {\n                field: "UAT application data zero fill",\n                details: format!(\n                    "unused application-data bytes after offset {offset} must be zero"\n                ),\n            });\n        }\n\n        Ok(frames)\n    }''',
)

replace_once(
    "src/uplink.rs",
    '''    /// Decodes every FIS-B APDU information frame in this uplink payload.\n    pub fn apdus(&self) -> Result<Vec<Apdu>> {''',
    '''    /// Decodes every FIS-B APDU information frame without discarding future\n    /// optional Product Descriptor forms. Known minimal headers become parsed\n    /// APDUs; optional descriptors are preserved losslessly as opaque payloads.\n    pub fn apdu_payloads(&self) -> Result<Vec<ApduPayload>> {\n        self.information_frames()?\n            .into_iter()\n            .filter(|frame| frame.frame_type == FrameType::FisBApdu)\n            .map(|frame| frame.apdu_payload())\n            .collect()\n    }\n\n    /// Strict semantic decoding for the currently implemented minimal UAT APDU\n    /// profile. Use `apdu_payloads` when lossless forward compatibility matters.\n    pub fn apdus(&self) -> Result<Vec<Apdu>> {''',
)
replace_once(
    "src/uplink.rs",
    '''    pub fn apdu(&self) -> Result<Apdu> {\n        if self.frame_type != FrameType::FisBApdu {''',
    '''    pub fn apdu_payload(&self) -> Result<ApduPayload> {\n        if self.frame_type != FrameType::FisBApdu {\n            return Err(Gdl90Error::InvalidField {\n                field: "frame type",\n                details: "frame does not contain a FIS-B APDU".to_string(),\n            });\n        }\n        ApduPayload::decode(&self.data)\n    }\n\n    pub fn apdu(&self) -> Result<Apdu> {\n        if self.frame_type != FrameType::FisBApdu {''',
)

replace_once(
    "src/uplink.rs",
    '''#[derive(Debug, Clone, PartialEq, Eq)]\npub struct Apdu {''',
    '''#[derive(Debug, Clone, PartialEq, Eq)]\npub struct OpaqueApdu {\n    pub product_id: u16,\n    pub descriptor_flags: u8,\n    pub raw: Vec<u8>,\n}\n\n#[derive(Debug, Clone, PartialEq, Eq)]\npub enum ApduPayload {\n    Parsed(Apdu),\n    OpaqueOptionalDescriptor(OpaqueApdu),\n}\n\nimpl ApduPayload {\n    pub fn decode(bytes: &[u8]) -> Result<Self> {\n        if bytes.len() < MIN_APDU_HEADER_LEN {\n            return Err(Gdl90Error::InvalidLength {\n                context: "APDU",\n                expected: "at least 4 bytes",\n                actual: bytes.len(),\n            });\n        }\n        if bytes.len() > MAX_APDU_LEN {\n            return Err(Gdl90Error::InvalidLength {\n                context: "APDU",\n                expected: "at most 422 bytes",\n                actual: bytes.len(),\n            });\n        }\n\n        let descriptor_flags = bytes[0] >> 5;\n        let product_id = (u16::from(bytes[0] & 0x1F) << 6) | u16::from(bytes[1] >> 2);\n        if descriptor_flags == 0 {\n            Ok(Self::Parsed(Apdu::decode(bytes)?))\n        } else {\n            Ok(Self::OpaqueOptionalDescriptor(OpaqueApdu {\n                product_id,\n                descriptor_flags,\n                raw: bytes.to_vec(),\n            }))\n        }\n    }\n\n    pub fn encode(&self) -> Result<Vec<u8>> {\n        match self {\n            Self::Parsed(apdu) => apdu.encode(),\n            Self::OpaqueOptionalDescriptor(apdu) => {\n                if apdu.raw.len() < MIN_APDU_HEADER_LEN || apdu.raw.len() > MAX_APDU_LEN {\n                    return Err(Gdl90Error::InvalidLength {\n                        context: "opaque APDU",\n                        expected: "4..=422 bytes",\n                        actual: apdu.raw.len(),\n                    });\n                }\n                Ok(apdu.raw.clone())\n            }\n        }\n    }\n\n    pub fn product_id(&self) -> u16 {\n        match self {\n            Self::Parsed(apdu) => apdu.header.product_id,\n            Self::OpaqueOptionalDescriptor(apdu) => apdu.product_id,\n        }\n    }\n}\n\n#[derive(Debug, Clone, PartialEq, Eq)]\npub struct Apdu {''',
)

replace_regex(
    "src/uplink.rs",
    r'''#\[derive\(Debug, Clone\)\]\nstruct PendingProductFile \{.*?\n\}\n\nfn assemble_product_file''',
    r'''pub const DEFAULT_REASSEMBLY_MAX_PENDING_FILES: usize = 64;
pub const DEFAULT_REASSEMBLY_MAX_BUFFERED_BYTES: usize = 1_048_576;
pub const DEFAULT_REASSEMBLY_MAX_SEGMENTS_PER_FILE: usize = 511;
pub const DEFAULT_REASSEMBLY_MAX_AGE: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReassemblyLimits {
    pub max_pending_files: usize,
    pub max_buffered_bytes: usize,
    pub max_segments_per_file: usize,
    pub max_age: Duration,
}

impl Default for ReassemblyLimits {
    fn default() -> Self {
        Self {
            max_pending_files: DEFAULT_REASSEMBLY_MAX_PENDING_FILES,
            max_buffered_bytes: DEFAULT_REASSEMBLY_MAX_BUFFERED_BYTES,
            max_segments_per_file: DEFAULT_REASSEMBLY_MAX_SEGMENTS_PER_FILE,
            max_age: DEFAULT_REASSEMBLY_MAX_AGE,
        }
    }
}

impl ReassemblyLimits {
    fn validate(self) -> Result<Self> {
        if self.max_pending_files == 0
            || self.max_buffered_bytes == 0
            || self.max_segments_per_file == 0
            || self.max_age.is_zero()
        {
            return Err(Gdl90Error::InvalidField {
                field: "APDU reassembly limits",
                details: "all limits must be greater than zero".to_string(),
            });
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ApduReassemblyScope {
    pub source_id: u64,
    pub key: ApduProductFileKey,
}

#[derive(Debug, Clone)]
struct PendingProductFile {
    total: u16,
    segments: Vec<Option<Apdu>>,
    buffered_bytes: usize,
    last_update: Instant,
}

/// Bounded stateful reassembler for FIS-B product files split across APDUs.
///
/// State is scoped by a caller-provided source id, expires after `max_age`, and
/// is bounded by both file count and encoded byte count. The legacy `push`
/// method uses source id zero for single-source applications.
#[derive(Debug, Clone)]
pub struct ApduReassembler {
    pending: BTreeMap<ApduReassemblyScope, PendingProductFile>,
    limits: ReassemblyLimits,
    buffered_bytes: usize,
}

impl Default for ApduReassembler {
    fn default() -> Self {
        Self {
            pending: BTreeMap::new(),
            limits: ReassemblyLimits::default(),
            buffered_bytes: 0,
        }
    }
}

impl ApduReassembler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_limits(limits: ReassemblyLimits) -> Result<Self> {
        Ok(Self {
            limits: limits.validate()?,
            ..Self::default()
        })
    }

    pub fn limits(&self) -> ReassemblyLimits {
        self.limits
    }

    pub fn pending_file_count(&self) -> usize {
        self.pending.len()
    }

    pub fn buffered_bytes(&self) -> usize {
        self.buffered_bytes
    }

    pub fn pending_keys(&self) -> Vec<ApduProductFileKey> {
        self.pending.keys().map(|scope| scope.key).collect()
    }

    pub fn pending_scopes(&self) -> Vec<ApduReassemblyScope> {
        self.pending.keys().copied().collect()
    }

    pub fn discard(&mut self, key: ApduProductFileKey) -> bool {
        let scopes = self
            .pending
            .keys()
            .filter(|scope| scope.key == key)
            .copied()
            .collect::<Vec<_>>();
        let removed = !scopes.is_empty();
        for scope in scopes {
            self.remove_scope(scope);
        }
        removed
    }

    pub fn discard_scope(&mut self, scope: ApduReassemblyScope) -> bool {
        self.remove_scope(scope).is_some()
    }

    pub fn clear(&mut self) {
        self.pending.clear();
        self.buffered_bytes = 0;
    }

    pub fn expire(&mut self) -> usize {
        self.expire_at(Instant::now())
    }

    pub fn expire_at(&mut self, now: Instant) -> usize {
        let expired = self
            .pending
            .iter()
            .filter_map(|(scope, file)| {
                now.checked_duration_since(file.last_update)
                    .filter(|age| *age >= self.limits.max_age)
                    .map(|_| *scope)
            })
            .collect::<Vec<_>>();
        let count = expired.len();
        for scope in expired {
            self.remove_scope(scope);
        }
        count
    }

    pub fn push(&mut self, apdu: Apdu) -> Result<ReassemblyStatus> {
        self.push_from_at(0, apdu, Instant::now())
    }

    pub fn push_from(&mut self, source_id: u64, apdu: Apdu) -> Result<ReassemblyStatus> {
        self.push_from_at(source_id, apdu, Instant::now())
    }

    pub fn push_from_at(
        &mut self,
        source_id: u64,
        apdu: Apdu,
        now: Instant,
    ) -> Result<ReassemblyStatus> {
        self.expire_at(now);
        apdu.header.validate_operational_uat()?;

        if !apdu.header.segmentation_flag {
            return Ok(ReassemblyStatus::Unsegmented(apdu));
        }

        let segmentation = apdu.header.segmentation.ok_or(Gdl90Error::InvalidField {
            field: "APDU segmentation",
            details: "segmentation flag is set without a segmentation block".to_string(),
        })?;
        let key = ApduProductFileKey {
            product_id: apdu.header.product_id,
            product_file_id: segmentation.product_file_id,
        };
        let scope = ApduReassemblyScope { source_id, key };
        let total = segmentation.product_file_length;
        let total_usize = usize::from(total);
        if total_usize > self.limits.max_segments_per_file {
            return Err(Gdl90Error::ResourceLimit {
                resource: "APDU segments per product file",
                limit: self.limits.max_segments_per_file,
            });
        }
        let index = usize::from(segmentation.apdu_number - 1);
        let encoded_len = apdu.encode()?.len();

        let inserted_new = if self.pending.contains_key(&scope) {
            false
        } else {
            if self.pending.len() >= self.limits.max_pending_files {
                return Err(Gdl90Error::ResourceLimit {
                    resource: "pending APDU product files",
                    limit: self.limits.max_pending_files,
                });
            }
            self.pending.insert(
                scope,
                PendingProductFile {
                    total,
                    segments: vec![None; total_usize],
                    buffered_bytes: 0,
                    last_update: now,
                },
            );
            true
        };

        let existing = self
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

        let duplicate = match &existing.segments[index] {
            Some(stored) if stored == &apdu => true,
            Some(_) => {
                return Err(Gdl90Error::InvalidField {
                    field: "APDU retransmission",
                    details: format!(
                        "product file {key:?} received conflicting data for APDU {}",
                        segmentation.apdu_number
                    ),
                });
            }
            None => false,
        };

        if !duplicate {
            let next_total = self.buffered_bytes.checked_add(encoded_len).ok_or(
                Gdl90Error::ResourceLimit {
                    resource: "buffered APDU bytes",
                    limit: self.limits.max_buffered_bytes,
                },
            )?;
            if next_total > self.limits.max_buffered_bytes {
                if inserted_new {
                    self.remove_scope(scope);
                }
                return Err(Gdl90Error::ResourceLimit {
                    resource: "buffered APDU bytes",
                    limit: self.limits.max_buffered_bytes,
                });
            }
            let file = self.pending.get_mut(&scope).ok_or(Gdl90Error::InvalidField {
                field: "APDU product file",
                details: "pending state disappeared unexpectedly".to_string(),
            })?;
            file.segments[index] = Some(apdu);
            file.buffered_bytes += encoded_len;
            file.last_update = now;
            self.buffered_bytes = next_total;
        } else if let Some(file) = self.pending.get_mut(&scope) {
            file.last_update = now;
        }

        let file = self.pending.get(&scope).ok_or(Gdl90Error::InvalidField {
            field: "APDU product file",
            details: "pending state disappeared unexpectedly".to_string(),
        })?;
        let received = file
            .segments
            .iter()
            .filter(|segment| segment.is_some())
            .count() as u16;
        let complete_segments = if received == total {
            let mut complete = Vec::with_capacity(file.segments.len());
            for segment in &file.segments {
                complete.push(segment.clone().ok_or(Gdl90Error::InvalidField {
                    field: "APDU product file",
                    details: "received count is inconsistent with stored segments".to_string(),
                })?);
            }
            Some(complete)
        } else {
            None
        };

        if let Some(segments) = complete_segments {
            self.remove_scope(scope);
            return Ok(ReassemblyStatus::Complete(assemble_product_file(
                key, &segments,
            )?));
        }

        if duplicate {
            Ok(ReassemblyStatus::Duplicate {
                key,
                apdu_number: segmentation.apdu_number,
                received,
                total,
            })
        } else {
            Ok(ReassemblyStatus::Pending {
                key,
                received,
                total,
            })
        }
    }

    fn remove_scope(&mut self, scope: ApduReassemblyScope) -> Option<PendingProductFile> {
        let removed = self.pending.remove(&scope)?;
        self.buffered_bytes = self.buffered_bytes.saturating_sub(removed.buffered_bytes);
        Some(removed)
    }
}

fn assemble_product_file''',
)

replace_once(
    "src/analysis.rs",
    "pub fn validate_datagrams(datagrams: &[RecordedDatagram]) -> SessionValidation {",
    "/// Performs syntactic frame/message decoding only; it is not an interoperability or certification check.\npub fn validate_datagrams_syntax(datagrams: &[RecordedDatagram]) -> SessionValidation {",
)
analysis = read("src/analysis.rs")
analysis += '''\n/// Backward-compatible alias for syntactic validation.\npub fn validate_datagrams(datagrams: &[RecordedDatagram]) -> SessionValidation {\n    validate_datagrams_syntax(datagrams)\n}\n'''
write("src/analysis.rs", analysis)

replace_once(
    "src/lib.rs",
    "    VerticalFigureOfMerit,\n};",
    "    VerticalFigureOfMerit, VerticalFigureOfMeritEncoding,\n};",
)
replace_once(
    "src/lib.rs",
    '''    Apdu, ApduHeader, ApduMonthDay, ApduProductFileKey, ApduReassembler, ApduSegmentation,\n    CurrentReportList, CurrentReportListItem, FisbProduct, FisbProductId, FrameType,''',
    '''    Apdu, ApduHeader, ApduMonthDay, ApduPayload, ApduProductFileKey, ApduReassembler,\n    ApduReassemblyScope, ApduSegmentation, CurrentReportList, CurrentReportListItem, FisbProduct,\n    FisbProductId, FrameType, OpaqueApdu, ReassemblyLimits,''',
)

protocol_tests = read("tests/protocol.rs")
protocol_tests = protocol_tests.replace(".basic_payload()", ".basic_payload().unwrap()")
protocol_tests = protocol_tests.replace(".long_payload()", ".long_payload().unwrap()")
protocol_tests = protocol_tests.replace(
    "use gdl90::control::{",
    "use std::time::{Duration, Instant};\n\nuse gdl90::control::{",
    1,
)
protocol_tests = protocol_tests.replace(
    "    NexradBlock, NexradBlockReference, NexradIntensity, ReassemblyStatus, ReassemblyStrategy,",
    "    ApduPayload, NexradBlock, NexradBlockReference, NexradIntensity, ReassemblyLimits,\n    ReassemblyStatus, ReassemblyStrategy,",
    1,
)
protocol_tests += r'''

#[test]
fn malformed_pass_through_payload_types_are_rejected_without_panicking() {
    let mut basic = vec![30, 0xFF, 0xFF, 0xFF];
    let mut wrong_basic_payload = [0u8; 18];
    wrong_basic_payload[0] = 1 << 3;
    basic.extend_from_slice(&wrong_basic_payload);
    assert!(matches!(
        Message::decode(&basic),
        Err(gdl90::Gdl90Error::InvalidField {
            field: "Basic UAT payload type code",
            ..
        })
    ));

    let manually_constructed = Message::BasicReport(PassThroughReport {
        time_of_reception: None,
        payload: wrong_basic_payload,
    });
    assert!(manually_constructed.summary().contains("invalid basic UAT payload"));

    let mut long = vec![31, 0xFF, 0xFF, 0xFF];
    let wrong_long_payload = [0u8; 34];
    long.extend_from_slice(&wrong_long_payload);
    assert!(matches!(
        Message::decode(&long),
        Err(gdl90::Gdl90Error::InvalidField {
            field: "Long UAT payload type code",
            ..
        })
    ));
}

#[test]
fn foreflight_vfom_conflict_is_explicit_and_round_trippable() {
    let message = OwnshipGeometricAltitude {
        altitude_feet: 1_000,
        vertical_warning: false,
        vertical_figure_of_merit: VerticalFigureOfMerit::GreaterThan32766,
    };
    let garmin = message.encode().unwrap();
    assert_eq!(u16::from_be_bytes([garmin[3], garmin[4]]) & 0x7FFF, 0x7FFE);
    let foreflight = message.encode_for_foreflight().unwrap();
    assert_eq!(
        u16::from_be_bytes([foreflight[3], foreflight[4]]) & 0x7FFF,
        0x7EEE
    );
    assert_eq!(
        OwnshipGeometricAltitude::decode(&foreflight)
            .unwrap()
            .vertical_figure_of_merit,
        VerticalFigureOfMerit::Meters(0x7EEE)
    );
    assert_eq!(
        OwnshipGeometricAltitude::decode_foreflight_compatible(&foreflight)
            .unwrap()
            .vertical_figure_of_merit,
        VerticalFigureOfMerit::GreaterThan32766
    );
}

#[test]
fn uplink_rejects_nonzero_bytes_after_information_frames() {
    let header = UatUplinkHeader {
        position_valid: false,
        latitude_deg: 0.0,
        longitude_deg: 0.0,
        utc_coupled: false,
        application_data_valid: true,
        slot_id: 0,
        tisb_site_id: 0,
    }
    .encode()
    .unwrap();
    let mut payload = UatUplinkPayload {
        header,
        application_data: [0; 424],
    };
    payload.application_data[10] = 1;
    assert!(matches!(
        payload.information_frames(),
        Err(gdl90::Gdl90Error::InvalidField {
            field: "UAT application data zero fill",
            ..
        })
    ));
}

#[test]
fn optional_product_descriptors_are_preserved_losslessly() {
    let raw = vec![0b1010_0000, 0, 0, 0, 1, 2, 3];
    let decoded = ApduPayload::decode(&raw).unwrap();
    match &decoded {
        ApduPayload::OpaqueOptionalDescriptor(opaque) => {
            assert_eq!(opaque.descriptor_flags, 0b101);
            assert_eq!(opaque.raw, raw);
        }
        other => panic!("expected opaque optional descriptor, got {other:?}"),
    }
    assert_eq!(decoded.encode().unwrap(), raw);
}

#[test]
fn reassembly_is_bounded_scoped_and_expiring() {
    let start = Instant::now();
    let mut reassembler = ApduReassembler::with_limits(ReassemblyLimits {
        max_pending_files: 2,
        max_buffered_bytes: 1_024,
        max_segments_per_file: 2,
        max_age: Duration::from_secs(1),
    })
    .unwrap();

    let first = segmented_apdu(413, 7, 2, 1, b"ONE");
    reassembler.push_from_at(10, first.clone(), start).unwrap();
    reassembler.push_from_at(20, first, start).unwrap();
    assert_eq!(reassembler.pending_file_count(), 2);
    assert_eq!(reassembler.pending_scopes().len(), 2);

    let third_source = segmented_apdu(413, 8, 2, 1, b"TWO");
    assert!(matches!(
        reassembler.push_from_at(30, third_source, start),
        Err(gdl90::Gdl90Error::ResourceLimit {
            resource: "pending APDU product files",
            limit: 2
        })
    ));

    assert_eq!(reassembler.expire_at(start + Duration::from_secs(2)), 2);
    assert_eq!(reassembler.pending_file_count(), 0);
    assert_eq!(reassembler.buffered_bytes(), 0);
}
'''
write("tests/protocol.rs", protocol_tests)
