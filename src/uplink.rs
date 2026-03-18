use crate::error::{Gdl90Error, Result};
pub const UAT_UPLINK_PAYLOAD_LEN: usize = 432;
pub const UAT_HEADER_LEN: usize = 8;
pub const APPLICATION_DATA_LEN: usize = 424;
pub const MAX_APDU_LEN: usize = 422;
pub const MIN_APDU_HEADER_LEN: usize = 4;
pub const MAX_APDU_PAYLOAD_LEN: usize = MAX_APDU_LEN - MIN_APDU_HEADER_LEN;
pub const GENERIC_TEXT_PRODUCT_ID: u16 = 413;
pub const NEXRAD_PRODUCT_ID: u16 = 63;
pub const DLAC_RECORD_SEPARATOR: char = '\u{001E}';

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UatUplinkPayload {
    pub header: [u8; UAT_HEADER_LEN],
    pub application_data: [u8; APPLICATION_DATA_LEN],
}

impl UatUplinkPayload {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != UAT_UPLINK_PAYLOAD_LEN {
            return Err(Gdl90Error::InvalidLength {
                context: "UAT uplink payload",
                expected: "432 bytes",
                actual: bytes.len(),
            });
        }

        let mut header = [0u8; UAT_HEADER_LEN];
        header.copy_from_slice(&bytes[..UAT_HEADER_LEN]);

        let mut application_data = [0u8; APPLICATION_DATA_LEN];
        application_data.copy_from_slice(&bytes[UAT_HEADER_LEN..]);

        Ok(Self {
            header,
            application_data,
        })
    }

    pub fn as_bytes(&self) -> [u8; UAT_UPLINK_PAYLOAD_LEN] {
        let mut out = [0u8; UAT_UPLINK_PAYLOAD_LEN];
        out[..UAT_HEADER_LEN].copy_from_slice(&self.header);
        out[UAT_HEADER_LEN..].copy_from_slice(&self.application_data);
        out
    }

    pub fn information_frames(&self) -> Result<Vec<InformationFrame>> {
        let mut frames = Vec::new();
        let mut offset = 0usize;

        while offset + 2 <= self.application_data.len() {
            let first = self.application_data[offset];
            let second = self.application_data[offset + 1];
            let length = ((first as usize) << 1) | ((second as usize) >> 7);
            if length == 0 {
                break;
            }

            let total = length + 2;
            if offset + total > self.application_data.len() {
                return Err(Gdl90Error::InvalidField {
                    field: "I-Frame length",
                    details: format!(
                        "frame starting at byte {offset} overruns 424-byte application data"
                    ),
                });
            }

            let reserved = (second >> 4) & 0x07;
            let frame_type = FrameType::from_raw(second & 0x0F);
            let data = self.application_data[offset + 2..offset + total].to_vec();

            frames.push(InformationFrame {
                reserved,
                frame_type,
                data,
            });
            offset += total;
        }

        Ok(frames)
    }

    pub fn from_information_frames(
        header: [u8; UAT_HEADER_LEN],
        frames: &[InformationFrame],
    ) -> Result<Self> {
        let mut application_data = [0u8; APPLICATION_DATA_LEN];
        let mut offset = 0usize;

        for frame in frames {
            let length = frame.data.len();
            if length > 422 {
                return Err(Gdl90Error::InvalidField {
                    field: "I-Frame data length",
                    details: format!("{length} exceeds 422-byte maximum"),
                });
            }

            let total = length + 2;
            if offset + total > APPLICATION_DATA_LEN {
                return Err(Gdl90Error::InvalidField {
                    field: "application data",
                    details: "frames do not fit in the 424-byte application area".to_string(),
                });
            }

            application_data[offset] = ((length >> 1) & 0xFF) as u8;
            application_data[offset + 1] = (((length & 0x01) as u8) << 7)
                | ((frame.reserved & 0x07) << 4)
                | frame.frame_type.raw();
            application_data[offset + 2..offset + total].copy_from_slice(&frame.data);
            offset += total;
        }

        Ok(Self {
            header,
            application_data,
        })
    }

    pub fn fisb_products(&self) -> Result<Vec<FisbProduct>> {
        let mut products = Vec::new();
        for frame in self.information_frames()? {
            if frame.frame_type != FrameType::FisBApdu {
                continue;
            }
            products.push(frame.apdu()?.decode_product()?);
        }
        Ok(products)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameType {
    FisBApdu,
    Reserved(u8),
    Developmental,
}

impl FrameType {
    pub fn from_raw(value: u8) -> Self {
        match value {
            0x0 => Self::FisBApdu,
            0xF => Self::Developmental,
            other => Self::Reserved(other),
        }
    }

    pub fn raw(self) -> u8 {
        match self {
            Self::FisBApdu => 0x0,
            Self::Developmental => 0xF,
            Self::Reserved(value) => value & 0x0F,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InformationFrame {
    pub reserved: u8,
    pub frame_type: FrameType,
    pub data: Vec<u8>,
}

impl InformationFrame {
    pub fn apdu(&self) -> Result<Apdu> {
        if self.frame_type != FrameType::FisBApdu {
            return Err(Gdl90Error::InvalidField {
                field: "frame type",
                details: "frame does not contain a FIS-B APDU".to_string(),
            });
        }
        Apdu::from_bytes(&self.data)
    }

    pub fn from_apdu(apdu: &Apdu) -> Result<Self> {
        Ok(Self {
            reserved: 0,
            frame_type: FrameType::FisBApdu,
            data: apdu.to_bytes()?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Apdu {
    pub header: ApduHeader,
    pub payload: Vec<u8>,
}

impl Apdu {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < MIN_APDU_HEADER_LEN {
            return Err(Gdl90Error::InvalidLength {
                context: "APDU",
                expected: "at least 4 bytes",
                actual: bytes.len(),
            });
        }
        if bytes.len() > MAX_APDU_LEN {
            return Err(Gdl90Error::InvalidLength {
                context: "APDU",
                expected: "at most 422 bytes",
                actual: bytes.len(),
            });
        }

        let mut header = [0u8; 4];
        header.copy_from_slice(&bytes[..4]);
        let header = ApduHeader::from_bytes(header);
        header.validate_supported_by_current_parser()?;

        let apdu = Self {
            header,
            payload: bytes[4..].to_vec(),
        };
        apdu.validate_payload_len()?;
        Ok(apdu)
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        self.header.validate_supported_by_current_parser()?;
        self.validate_payload_len()?;

        let mut out = Vec::with_capacity(4 + self.payload.len());
        out.extend_from_slice(&self.header.to_bytes()?);
        out.extend_from_slice(&self.payload);
        Ok(out)
    }

    fn validate_payload_len(&self) -> Result<()> {
        if self.payload.len() > MAX_APDU_PAYLOAD_LEN {
            return Err(Gdl90Error::InvalidLength {
                context: "APDU payload",
                expected: "at most 418 bytes",
                actual: self.payload.len(),
            });
        }
        Ok(())
    }

    pub fn as_generic_text(&self) -> Result<GenericTextApdu> {
        if self.header.product_id != GENERIC_TEXT_PRODUCT_ID {
            return Err(Gdl90Error::InvalidField {
                field: "product id",
                details: format!(
                    "expected generic text product id {GENERIC_TEXT_PRODUCT_ID}, got {}",
                    self.header.product_id
                ),
            });
        }
        GenericTextApdu::from_apdu(self)
    }

    pub fn as_nexrad(&self) -> Result<NexradApdu> {
        if self.header.product_id != NEXRAD_PRODUCT_ID {
            return Err(Gdl90Error::InvalidField {
                field: "product id",
                details: format!(
                    "expected NEXRAD product id {NEXRAD_PRODUCT_ID}, got {}",
                    self.header.product_id
                ),
            });
        }
        NexradApdu::from_apdu(self)
    }

    pub fn decode_product(&self) -> Result<FisbProduct> {
        match self.header.product_id {
            GENERIC_TEXT_PRODUCT_ID => Ok(FisbProduct::GenericText(self.as_generic_text()?)),
            NEXRAD_PRODUCT_ID => Ok(FisbProduct::Nexrad(self.as_nexrad()?)),
            _ => Ok(FisbProduct::Unknown(self.clone())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FisbProduct {
    GenericText(GenericTextApdu),
    Nexrad(NexradApdu),
    Unknown(Apdu),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApduHeader {
    pub application_flag: bool,
    pub geo_flag: bool,
    pub product_file_flag: bool,
    pub product_id: u16,
    pub segmentation_flag: bool,
    pub time_option: u8,
    pub hours: u8,
    pub minutes: u8,
}

impl ApduHeader {
    pub fn validate(self) -> Result<()> {
        if self.product_id > 0x07FF {
            return Err(Gdl90Error::InvalidField {
                field: "APDU product id",
                details: "must fit in 11 bits".to_string(),
            });
        }
        if self.time_option > 0x03 {
            return Err(Gdl90Error::InvalidField {
                field: "APDU time option",
                details: "must fit in 2 bits".to_string(),
            });
        }
        if self.hours > 0x1F {
            return Err(Gdl90Error::InvalidField {
                field: "APDU hours",
                details: "must fit in 5 bits".to_string(),
            });
        }
        if self.minutes > 0x3F {
            return Err(Gdl90Error::InvalidField {
                field: "APDU minutes",
                details: "must fit in 6 bits".to_string(),
            });
        }
        Ok(())
    }

    pub fn from_bytes(bytes: [u8; 4]) -> Self {
        let word = u32::from_be_bytes(bytes);
        Self {
            application_flag: ((word >> 31) & 0x01) != 0,
            geo_flag: ((word >> 30) & 0x01) != 0,
            product_file_flag: ((word >> 29) & 0x01) != 0,
            product_id: ((word >> 18) & 0x07FF) as u16,
            segmentation_flag: ((word >> 17) & 0x01) != 0,
            time_option: ((word >> 15) & 0x03) as u8,
            hours: ((word >> 10) & 0x1F) as u8,
            minutes: ((word >> 4) & 0x3F) as u8,
        }
    }

    pub fn has_product_descriptor_options(self) -> bool {
        self.application_flag || self.geo_flag || self.product_file_flag
    }

    pub fn validate_supported_by_current_parser(self) -> Result<()> {
        self.validate()?;

        if self.has_product_descriptor_options() {
            return Err(Gdl90Error::InvalidField {
                field: "APDU product descriptor options",
                details: "optional product descriptor fields are not implemented".to_string(),
            });
        }
        if self.segmentation_flag {
            return Err(Gdl90Error::InvalidField {
                field: "APDU segmentation",
                details: "segmented APDUs are not implemented".to_string(),
            });
        }

        Ok(())
    }

    pub fn validate_minimal_uat(self) -> Result<()> {
        self.validate_supported_by_current_parser()?;
        if self.time_option != 0 {
            return Err(Gdl90Error::InvalidField {
                field: "APDU time option",
                details: "documented minimal UAT APDU headers use time option 0".to_string(),
            });
        }
        Ok(())
    }

    pub fn to_bytes(self) -> Result<[u8; 4]> {
        self.validate()?;

        let mut word = 0u32;
        word |= (self.application_flag as u32) << 31;
        word |= (self.geo_flag as u32) << 30;
        word |= (self.product_file_flag as u32) << 29;
        word |= (u32::from(self.product_id & 0x07FF)) << 18;
        word |= (self.segmentation_flag as u32) << 17;
        word |= (u32::from(self.time_option & 0x03)) << 15;
        word |= (u32::from(self.hours & 0x1F)) << 10;
        word |= (u32::from(self.minutes & 0x3F)) << 4;
        Ok(word.to_be_bytes())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenericTextApdu {
    pub header: ApduHeader,
    pub records: Vec<GenericTextRecord>,
}

impl GenericTextApdu {
    pub fn from_apdu(apdu: &Apdu) -> Result<Self> {
        let text = decode_dlac(&apdu.payload);
        let mut records = Vec::new();
        for raw in text.split(DLAC_RECORD_SEPARATOR) {
            let trimmed = raw.trim_matches('\0');
            if trimmed.is_empty() {
                continue;
            }
            records.push(GenericTextRecord::parse(trimmed)?);
        }
        let decoded = Self {
            header: apdu.header,
            records,
        };
        decoded.validate()?;
        Ok(decoded)
    }

    pub fn to_apdu(&self) -> Result<Apdu> {
        self.validate()?;

        let mut text = String::new();
        for record in &self.records {
            text.push_str(&record.render());
            text.push(DLAC_RECORD_SEPARATOR);
        }
        let apdu = Apdu {
            header: self.header,
            payload: encode_dlac(&text)?,
        };
        apdu.validate_payload_len()?;
        Ok(apdu)
    }

    pub fn validate(&self) -> Result<()> {
        if self.header.product_id != GENERIC_TEXT_PRODUCT_ID {
            return Err(Gdl90Error::InvalidField {
                field: "product id",
                details: format!(
                    "expected generic text product id {GENERIC_TEXT_PRODUCT_ID}, got {}",
                    self.header.product_id
                ),
            });
        }
        self.header.validate_minimal_uat()?;
        if self.records.is_empty() {
            return Err(Gdl90Error::InvalidField {
                field: "generic text APDU",
                details: "must contain at least one text record".to_string(),
            });
        }

        let mut total_encoded_len = 0usize;
        for record in &self.records {
            record.validate_metar_taf_composition()?;
            total_encoded_len += encoded_generic_text_record_len(record)?;
        }
        if total_encoded_len > MAX_APDU_PAYLOAD_LEN {
            return Err(Gdl90Error::InvalidLength {
                context: "Generic Text APDU payload",
                expected: "at most 418 bytes",
                actual: total_encoded_len,
            });
        }
        Ok(())
    }

    pub fn validate_records(&self) -> Result<()> {
        self.validate()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextQualifier {
    SpecialReport,
    Amendment,
}

impl TextQualifier {
    fn as_str(self) -> &'static str {
        match self {
            Self::SpecialReport => "SP",
            Self::Amendment => "AM",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenericTextRecordKind {
    Metar,
    Taf,
    Other,
}

impl GenericTextRecordKind {
    pub fn from_record_type(record_type: &str) -> Self {
        match record_type {
            "METAR" => Self::Metar,
            "TAF" => Self::Taf,
            _ => Self::Other,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GenericTextField {
    Text(String),
    Nil,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenericTextRecord {
    pub kind: GenericTextRecordKind,
    pub record_type: String,
    pub location: GenericTextField,
    pub record_time: GenericTextField,
    pub qualifier: Option<TextQualifier>,
    pub text: String,
}

impl GenericTextRecord {
    pub fn parse(raw: &str) -> Result<Self> {
        let mut parts = raw.splitn(5, ' ');
        let record_type = parts
            .next()
            .filter(|value| !value.is_empty())
            .ok_or(Gdl90Error::InvalidField {
                field: "generic text record",
                details: "missing record type".to_string(),
            })?
            .to_string();
        let location = parts
            .next()
            .filter(|value| !value.is_empty())
            .ok_or(Gdl90Error::InvalidField {
                field: "generic text record",
                details: "missing location".to_string(),
            })?
            .to_string();
        let record_time = parts
            .next()
            .filter(|value| !value.is_empty())
            .ok_or(Gdl90Error::InvalidField {
                field: "generic text record",
                details: "missing record time".to_string(),
            })?
            .to_string();
        let kind = GenericTextRecordKind::from_record_type(&record_type);

        let fourth = parts.next().ok_or(Gdl90Error::InvalidField {
            field: "generic text record",
            details: "missing report body".to_string(),
        })?;

        let (qualifier, text) = match fourth {
            "SP" => (
                Some(TextQualifier::SpecialReport),
                parts.next().unwrap_or_default().to_string(),
            ),
            "AM" => (
                Some(TextQualifier::Amendment),
                parts.next().unwrap_or_default().to_string(),
            ),
            _ => {
                let remainder = if let Some(rest) = parts.next() {
                    format!("{fourth} {rest}")
                } else {
                    fourth.to_string()
                };
                (None, remainder)
            }
        };

        Ok(Self {
            kind,
            record_type,
            location: parse_generic_text_field(location),
            record_time: parse_generic_text_field(record_time),
            qualifier,
            text,
        })
    }

    pub fn render(&self) -> String {
        let mut out = format!(
            "{} {} {}",
            self.record_type,
            render_generic_text_field(&self.location),
            render_generic_text_field(&self.record_time)
        );
        if let Some(qualifier) = self.qualifier {
            out.push(' ');
            out.push_str(qualifier.as_str());
        }
        out.push(' ');
        out.push_str(&self.text);
        out
    }

    pub fn validate_metar_taf_composition(&self) -> Result<()> {
        match self.kind {
            GenericTextRecordKind::Metar | GenericTextRecordKind::Taf => {
                if self.text.contains(DLAC_RECORD_SEPARATOR) {
                    return Err(Gdl90Error::InvalidField {
                        field: "generic text record text",
                        details: "text report must not contain record separator".to_string(),
                    });
                }

                let qualifier_ok = matches!(
                    (self.kind, self.qualifier),
                    (
                        GenericTextRecordKind::Metar,
                        None | Some(TextQualifier::SpecialReport)
                    ) | (
                        GenericTextRecordKind::Taf,
                        None | Some(TextQualifier::Amendment)
                    )
                );
                if !qualifier_ok {
                    return Err(Gdl90Error::InvalidField {
                        field: "generic text qualifier",
                        details: "qualifier does not match METAR/TAF rules".to_string(),
                    });
                }

                let location_ok = matches!(
                    &self.location,
                    GenericTextField::Text(value) if !value.contains(' ') && !value.is_empty()
                ) || matches!(&self.location, GenericTextField::Nil);
                let time_ok = matches!(
                    &self.record_time,
                    GenericTextField::Text(value) if !value.contains(' ') && !value.is_empty()
                ) || matches!(&self.record_time, GenericTextField::Nil);

                if !location_ok || !time_ok {
                    return Err(Gdl90Error::InvalidField {
                        field: "generic text METAR/TAF structure",
                        details: "location/time must be token fields or NIL".to_string(),
                    });
                }
            }
            GenericTextRecordKind::Other => {}
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NexradApdu {
    pub header: ApduHeader,
    pub block: NexradBlock,
}

impl NexradApdu {
    pub fn from_apdu(apdu: &Apdu) -> Result<Self> {
        let decoded = Self {
            header: apdu.header,
            block: NexradBlock::from_payload(&apdu.payload)?,
        };
        decoded.validate()?;
        Ok(decoded)
    }

    pub fn to_apdu(&self) -> Result<Apdu> {
        self.validate()?;

        let apdu = Apdu {
            header: self.header,
            payload: self.block.to_payload(),
        };
        apdu.validate_payload_len()?;
        Ok(apdu)
    }

    pub fn validate(&self) -> Result<()> {
        if self.header.product_id != NEXRAD_PRODUCT_ID {
            return Err(Gdl90Error::InvalidField {
                field: "product id",
                details: format!(
                    "expected NEXRAD product id {NEXRAD_PRODUCT_ID}, got {}",
                    self.header.product_id
                ),
            });
        }
        self.header.validate_minimal_uat()?;
        self.block.validate()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NexradBlock {
    Empty {
        block_reference_indicator: [u8; 3],
    },
    RunLengthEncoded {
        block_reference_indicator: [u8; 3],
        runs: Vec<NexradRun>,
    },
    Unparsed {
        raw: Vec<u8>,
    },
}

impl NexradBlock {
    pub fn from_payload(payload: &[u8]) -> Result<Self> {
        if payload.len() < 3 {
            return Err(Gdl90Error::InvalidLength {
                context: "NEXRAD APDU payload",
                expected: "at least 3 bytes",
                actual: payload.len(),
            });
        }

        let mut reference = [0u8; 3];
        reference.copy_from_slice(&payload[..3]);
        if payload.len() == 3 {
            return Ok(Self::Empty {
                block_reference_indicator: reference,
            });
        }

        let mut runs = Vec::with_capacity(payload.len() - 3);
        for byte in &payload[3..] {
            runs.push(NexradRun {
                count: (byte >> 3) + 1,
                intensity: byte & 0x07,
            });
        }

        let total: usize = runs.iter().map(|run| run.count as usize).sum();
        if total != 128 {
            return Ok(Self::Unparsed {
                raw: payload.to_vec(),
            });
        }

        Ok(Self::RunLengthEncoded {
            block_reference_indicator: reference,
            runs,
        })
    }

    pub fn to_payload(&self) -> Vec<u8> {
        match self {
            Self::Empty {
                block_reference_indicator,
            } => block_reference_indicator.to_vec(),
            Self::RunLengthEncoded {
                block_reference_indicator,
                runs,
            } => {
                let mut out = Vec::with_capacity(3 + runs.len());
                out.extend_from_slice(block_reference_indicator);
                for run in runs {
                    out.push(((run.count - 1) << 3) | (run.intensity & 0x07));
                }
                out
            }
            Self::Unparsed { raw } => raw.clone(),
        }
    }

    pub fn validate(&self) -> Result<()> {
        match self {
            Self::Empty { .. } => Ok(()),
            Self::RunLengthEncoded { runs, .. } => {
                let total: usize = runs.iter().map(|run| run.count as usize).sum();
                if total != 128 {
                    return Err(Gdl90Error::InvalidField {
                        field: "NEXRAD run-length block",
                        details: format!("runs expand to {total} bins instead of 128"),
                    });
                }
                for run in runs {
                    if !(1..=32).contains(&run.count) {
                        return Err(Gdl90Error::InvalidField {
                            field: "NEXRAD run length",
                            details: format!("count {} is outside 1..=32", run.count),
                        });
                    }
                    if run.intensity > 7 {
                        return Err(Gdl90Error::InvalidField {
                            field: "NEXRAD intensity",
                            details: format!("intensity {} is outside 0..=7", run.intensity),
                        });
                    }
                }
                Ok(())
            }
            Self::Unparsed { raw } => {
                if raw.len() < 3 {
                    return Err(Gdl90Error::InvalidLength {
                        context: "NEXRAD APDU payload",
                        expected: "at least 3 bytes",
                        actual: raw.len(),
                    });
                }
                Ok(())
            }
        }
    }

    pub fn decode_bins(&self) -> Vec<u8> {
        match self {
            Self::Empty { .. } => vec![0u8; 128],
            Self::RunLengthEncoded { runs, .. } => {
                let mut bins = Vec::with_capacity(128);
                for run in runs {
                    bins.extend(std::iter::repeat_n(run.intensity, run.count as usize));
                }
                bins
            }
            Self::Unparsed { .. } => Vec::new(),
        }
    }

    pub fn decode_rows(&self) -> Vec<Vec<u8>> {
        self.decode_bins()
            .chunks(32)
            .map(|row| row.to_vec())
            .collect()
    }

    pub fn from_bins(block_reference_indicator: [u8; 3], bins: &[u8; 128]) -> Result<Self> {
        if bins.iter().all(|value| *value == 0) {
            return Ok(Self::Empty {
                block_reference_indicator,
            });
        }

        let mut runs = Vec::new();
        let mut current = bins[0];
        let mut count = 1u8;
        for value in bins.iter().skip(1) {
            if *value == current && count < 32 {
                count += 1;
            } else {
                runs.push(NexradRun {
                    count,
                    intensity: current,
                });
                current = *value;
                count = 1;
            }
        }
        runs.push(NexradRun {
            count,
            intensity: current,
        });

        Ok(Self::RunLengthEncoded {
            block_reference_indicator,
            runs,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NexradRun {
    pub count: u8,
    pub intensity: u8,
}

fn decode_dlac(bytes: &[u8]) -> String {
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let word = match chunk.len() {
            3 => u32::from_be_bytes([0, chunk[0], chunk[1], chunk[2]]),
            2 => u32::from_be_bytes([0, 0, chunk[0], chunk[1]]) << 8,
            1 => u32::from_be_bytes([0, 0, 0, chunk[0]]) << 16,
            _ => unreachable!(),
        };

        let count = match chunk.len() {
            3 => 4,
            2 => 2,
            1 => 1,
            _ => 0,
        };

        for index in 0..count {
            let shift = 18 - (index * 6);
            let value = ((word >> shift) & 0x3F) as u8;
            out.push(decode_dlac_char(value));
        }
    }
    out
}

fn parse_generic_text_field(value: String) -> GenericTextField {
    if value == "NIL=" {
        GenericTextField::Nil
    } else {
        GenericTextField::Text(value)
    }
}

fn render_generic_text_field(value: &GenericTextField) -> &str {
    match value {
        GenericTextField::Text(value) => value.as_str(),
        GenericTextField::Nil => "NIL=",
    }
}

fn encode_dlac(text: &str) -> Result<Vec<u8>> {
    let mut values = Vec::with_capacity(text.len());
    for ch in text.chars() {
        values.push(encode_dlac_char(ch)?);
    }

    let mut out = Vec::with_capacity((values.len() * 6).div_ceil(8));
    let mut index = 0usize;
    while index < values.len() {
        let a = values[index];
        let b = values.get(index + 1).copied().unwrap_or(0);
        let c = values.get(index + 2).copied().unwrap_or(0);
        let d = values.get(index + 3).copied().unwrap_or(0);
        let word = ((a as u32) << 18) | ((b as u32) << 12) | ((c as u32) << 6) | d as u32;
        out.push(((word >> 16) & 0xFF) as u8);
        if index + 1 < values.len() {
            out.push(((word >> 8) & 0xFF) as u8);
        }
        if index + 2 < values.len() {
            out.push((word & 0xFF) as u8);
        }
        index += 4;
    }
    Ok(out)
}

fn encoded_generic_text_record_len(record: &GenericTextRecord) -> Result<usize> {
    let mut text = record.render();
    text.push(DLAC_RECORD_SEPARATOR);
    let encoded = encode_dlac(&text)?;
    if encoded.len() > MAX_APDU_PAYLOAD_LEN {
        return Err(Gdl90Error::InvalidLength {
            context: "generic text record",
            expected: "at most 418 bytes",
            actual: encoded.len(),
        });
    }
    Ok(encoded.len())
}

fn decode_dlac_char(value: u8) -> char {
    match value {
        0 => '\0',
        1..=26 => (b'A' + (value - 1)) as char,
        27 => '\t',
        28 => '\n',
        29 => DLAC_RECORD_SEPARATOR,
        30 => '\r',
        31 => '\u{001F}',
        32..=63 => value as char,
        _ => unreachable!(),
    }
}

fn encode_dlac_char(ch: char) -> Result<u8> {
    match ch {
        '\0' => Ok(0),
        'A'..='Z' => Ok((ch as u8) - b'A' + 1),
        'a'..='z' => Ok((ch as u8).to_ascii_uppercase() - b'A' + 1),
        '\t' => Ok(27),
        '\n' => Ok(28),
        DLAC_RECORD_SEPARATOR => Ok(29),
        '\r' => Ok(30),
        '\u{001F}' => Ok(31),
        ' '..='?' => Ok(ch as u8),
        _ => Err(Gdl90Error::UnsupportedCharacter {
            context: "DLAC text",
            ch,
        }),
    }
}
