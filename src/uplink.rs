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
// EASA ETSO-C157a Appendix 1 refers to "Time Flag #1" and "Time Flag #2" without
// naming which bit corresponds to month/day versus seconds. This implementation
// maps flag #1 to month/day and flag #2 to seconds to match the amended Table D-1
// field order and the 13/19/22-bit header-time lengths.
pub const APDU_TIME_FLAG_SECONDS: u8 = 0b01;
pub const APDU_TIME_FLAG_MONTH_DAY: u8 = 0b10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FisbProductId {
    GenericText,
    Nexrad,
    Unknown(u16),
}

impl FisbProductId {
    pub fn from_raw(value: u16) -> Self {
        match value {
            GENERIC_TEXT_PRODUCT_ID => Self::GenericText,
            NEXRAD_PRODUCT_ID => Self::Nexrad,
            other => Self::Unknown(other),
        }
    }

    pub fn raw(self) -> u16 {
        match self {
            Self::GenericText => GENERIC_TEXT_PRODUCT_ID,
            Self::Nexrad => NEXRAD_PRODUCT_ID,
            Self::Unknown(value) => value,
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::GenericText => "Generic Text",
            Self::Nexrad => "NEXRAD",
            Self::Unknown(_) => "Unknown",
        }
    }
}

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

        let (header, header_len) = ApduHeader::decode(bytes)?;
        header.validate_supported_by_current_parser()?;

        let apdu = Self {
            header,
            payload: bytes[header_len..].to_vec(),
        };
        apdu.validate_payload_len()?;
        Ok(apdu)
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        self.header.validate_supported_by_current_parser()?;
        self.validate_payload_len()?;

        let header = self.header.to_bytes()?;
        let mut out = Vec::with_capacity(header.len() + self.payload.len());
        out.extend_from_slice(&header);
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
        match self.product_id() {
            FisbProductId::GenericText => Ok(FisbProduct::GenericText(self.as_generic_text()?)),
            FisbProductId::Nexrad => Ok(FisbProduct::Nexrad(self.as_nexrad()?)),
            FisbProductId::Unknown(_) => Ok(FisbProduct::Unknown(self.clone())),
        }
    }

    pub fn product_id(&self) -> FisbProductId {
        FisbProductId::from_raw(self.header.product_id)
    }

    pub fn product_name(&self) -> &'static str {
        self.product_id().display_name()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FisbProduct {
    GenericText(GenericTextApdu),
    Nexrad(NexradApdu),
    Unknown(Apdu),
}

impl FisbProduct {
    pub fn product_id(&self) -> FisbProductId {
        match self {
            Self::GenericText(_) => FisbProductId::GenericText,
            Self::Nexrad(_) => FisbProductId::Nexrad,
            Self::Unknown(apdu) => apdu.product_id(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApduHeader {
    pub application_flag: bool,
    pub geo_flag: bool,
    pub product_file_flag: bool,
    pub product_id: u16,
    pub segmentation_flag: bool,
    pub time_option: u8,
    pub month_day: Option<ApduMonthDay>,
    pub hours: u8,
    pub minutes: u8,
    pub seconds: Option<u8>,
    pub segmentation: Option<ApduSegmentation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApduMonthDay {
    pub month: u8,
    pub day: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApduSegmentation {
    pub product_file_id: u16,
    pub product_file_length: u16,
    pub apdu_number: u16,
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
        if self.time_option == 0x03 {
            return Err(Gdl90Error::InvalidField {
                field: "APDU time flags",
                details: "time flag #1 and time flag #2 cannot both be set".to_string(),
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
        if let Some(month_day) = self.month_day {
            if month_day.month > 0x0F {
                return Err(Gdl90Error::InvalidField {
                    field: "APDU month",
                    details: "must fit in 4 bits".to_string(),
                });
            }
            if month_day.day > 0x1F {
                return Err(Gdl90Error::InvalidField {
                    field: "APDU day",
                    details: "must fit in 5 bits".to_string(),
                });
            }
        }
        if let Some(seconds) = self.seconds {
            if seconds > 0x3F {
                return Err(Gdl90Error::InvalidField {
                    field: "APDU seconds",
                    details: "must fit in 6 bits".to_string(),
                });
            }
        }
        if let Some(segmentation) = self.segmentation {
            if segmentation.product_file_id > 0x03FF {
                return Err(Gdl90Error::InvalidField {
                    field: "APDU product file id",
                    details: "must fit in 10 bits".to_string(),
                });
            }
            if segmentation.product_file_length > 0x01FF {
                return Err(Gdl90Error::InvalidField {
                    field: "APDU product file length",
                    details: "must fit in 9 bits".to_string(),
                });
            }
            if segmentation.apdu_number > 0x01FF {
                return Err(Gdl90Error::InvalidField {
                    field: "APDU number",
                    details: "must fit in 9 bits".to_string(),
                });
            }
        }

        let has_month_day = self.month_day.is_some();
        let has_seconds = self.seconds.is_some();
        let has_segmentation = self.segmentation.is_some();
        if self.time_option != encode_apdu_time_option(has_month_day, has_seconds)? {
            return Err(Gdl90Error::InvalidField {
                field: "APDU time option",
                details: "time option bits do not match the optional time fields".to_string(),
            });
        }
        if self.segmentation_flag != has_segmentation {
            return Err(Gdl90Error::InvalidField {
                field: "APDU segmentation",
                details: "segmentation flag does not match the presence of a segmentation block"
                    .to_string(),
            });
        }
        Ok(())
    }

    pub fn decode(bytes: &[u8]) -> Result<(Self, usize)> {
        if bytes.len() < MIN_APDU_HEADER_LEN {
            return Err(Gdl90Error::InvalidLength {
                context: "APDU header",
                expected: "at least 4 bytes",
                actual: bytes.len(),
            });
        }

        let mut reader = BitReader::new(bytes);
        let application_flag = reader.read_bool()?;
        let geo_flag = reader.read_bool()?;
        let product_file_flag = reader.read_bool()?;
        let product_id = reader.read_u16(11)?;
        let segmentation_flag = reader.read_bool()?;
        let time_flag_1 = reader.read_bool()?;
        let time_flag_2 = reader.read_bool()?;
        let time_option = ((time_flag_1 as u8) << 1) | time_flag_2 as u8;

        let month_day = if time_flag_1 {
            Some(ApduMonthDay {
                month: reader.read_u8(4)?,
                day: reader.read_u8(5)?,
            })
        } else {
            None
        };

        let hours = reader.read_u8(5)?;
        let minutes = reader.read_u8(6)?;
        let seconds = if time_flag_2 {
            Some(reader.read_u8(6)?)
        } else {
            None
        };

        let segmentation = if segmentation_flag {
            Some(ApduSegmentation {
                product_file_id: reader.read_u16(10)?,
                product_file_length: reader.read_u16(9)?,
                apdu_number: reader.read_u16(9)?,
            })
        } else {
            None
        };

        reader.align_to_byte_zero_pad()?;
        let header_len = reader.bytes_consumed();
        let header = Self {
            application_flag,
            geo_flag,
            product_file_flag,
            product_id,
            segmentation_flag,
            time_option,
            month_day,
            hours,
            minutes,
            seconds,
            segmentation,
        };
        header.validate()?;
        Ok((header, header_len))
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
            month_day: None,
            hours: ((word >> 10) & 0x1F) as u8,
            minutes: ((word >> 4) & 0x3F) as u8,
            seconds: None,
            segmentation: None,
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

        Ok(())
    }

    pub fn validate_minimal_uat(self) -> Result<()> {
        self.validate_supported_by_current_parser()?;
        if self.time_option != 0 || self.seconds.is_some() || self.month_day.is_some() {
            return Err(Gdl90Error::InvalidField {
                field: "APDU time option",
                details: "documented minimal UAT APDU headers use time option 0".to_string(),
            });
        }
        if self.segmentation_flag || self.segmentation.is_some() {
            return Err(Gdl90Error::InvalidField {
                field: "APDU segmentation",
                details: "documented minimal UAT APDU headers do not include segmentation"
                    .to_string(),
            });
        }
        Ok(())
    }

    pub fn to_bytes(self) -> Result<Vec<u8>> {
        self.validate()?;

        let mut writer = BitWriter::new();
        writer.push_bool(self.application_flag);
        writer.push_bool(self.geo_flag);
        writer.push_bool(self.product_file_flag);
        writer.push_u16(self.product_id, 11);
        writer.push_bool(self.segmentation_flag);
        writer.push_bool((self.time_option & APDU_TIME_FLAG_MONTH_DAY) != 0);
        writer.push_bool((self.time_option & APDU_TIME_FLAG_SECONDS) != 0);
        if let Some(month_day) = self.month_day {
            writer.push_u8(month_day.month, 4);
            writer.push_u8(month_day.day, 5);
        }
        writer.push_u8(self.hours, 5);
        writer.push_u8(self.minutes, 6);
        if let Some(seconds) = self.seconds {
            writer.push_u8(seconds, 6);
        }
        if let Some(segmentation) = self.segmentation {
            writer.push_u16(segmentation.product_file_id, 10);
            writer.push_u16(segmentation.product_file_length, 9);
            writer.push_u16(segmentation.apdu_number, 9);
        }
        writer.finish_zero_padded()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenericTextApdu {
    pub header: ApduHeader,
    pub records: Vec<GenericTextRecord>,
}

impl GenericTextApdu {
    fn validate_header(header: ApduHeader) -> Result<()> {
        if header.product_id != GENERIC_TEXT_PRODUCT_ID {
            return Err(Gdl90Error::InvalidField {
                field: "product id",
                details: format!(
                    "expected generic text product id {GENERIC_TEXT_PRODUCT_ID}, got {}",
                    header.product_id
                ),
            });
        }
        header.validate_minimal_uat()?;
        Ok(())
    }

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

    pub fn pack_records(header: ApduHeader, records: &[GenericTextRecord]) -> Result<Vec<Self>> {
        Self::validate_header(header)?;
        if records.is_empty() {
            return Err(Gdl90Error::InvalidField {
                field: "generic text APDU",
                details: "must contain at least one text record".to_string(),
            });
        }

        let mut apdus = Vec::new();
        let mut current_records = Vec::new();
        let mut current_len = 0usize;

        for record in records {
            record.validate_metar_taf_composition()?;
            let encoded_len = record.encoded_len()?;

            if !current_records.is_empty() && current_len + encoded_len > MAX_APDU_PAYLOAD_LEN {
                apdus.push(Self {
                    header,
                    records: current_records,
                });
                current_records = Vec::new();
                current_len = 0;
            }

            current_records.push(record.clone());
            current_len += encoded_len;
        }

        if !current_records.is_empty() {
            apdus.push(Self {
                header,
                records: current_records,
            });
        }

        for apdu in &apdus {
            apdu.validate()?;
        }

        Ok(apdus)
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
        Self::validate_header(self.header)?;
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
        let encoded_len = self.encoded_len()?;
        if encoded_len > MAX_APDU_PAYLOAD_LEN {
            return Err(Gdl90Error::InvalidLength {
                context: "generic text record",
                expected: "at most 418 bytes",
                actual: encoded_len,
            });
        }

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

    pub fn encoded_len(&self) -> Result<usize> {
        encoded_generic_text_record_len(self)
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NexradBlockReference {
    pub is_run_length_encoded: bool,
    pub north: bool,
    pub scale: u8,
    pub block_number: u32,
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

    pub fn decode_intensity_bins(&self) -> Result<Vec<NexradIntensity>> {
        self.decode_bins()
            .into_iter()
            .map(NexradIntensity::from_encoded)
            .collect()
    }

    pub fn decode_intensity_rows(&self) -> Result<Vec<Vec<NexradIntensity>>> {
        let bins = self.decode_intensity_bins()?;
        Ok(bins.chunks(32).map(|row| row.to_vec()).collect())
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

    pub fn block_reference_indicator(&self) -> Option<[u8; 3]> {
        match self {
            Self::Empty {
                block_reference_indicator,
            }
            | Self::RunLengthEncoded {
                block_reference_indicator,
                ..
            } => Some(*block_reference_indicator),
            Self::Unparsed { raw } => raw
                .get(..3)
                .and_then(|slice| <[u8; 3]>::try_from(slice).ok()),
        }
    }

    pub fn block_reference(&self) -> Option<NexradBlockReference> {
        let raw = self.block_reference_indicator()?;
        Some(NexradBlockReference::from_raw(raw))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NexradRun {
    pub count: u8,
    pub intensity: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NexradIntensity {
    Value0,
    Value1,
    Value2,
    Value3,
    Value4,
    Value5,
    Value6,
    Value7,
}

impl NexradIntensity {
    pub fn from_encoded(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::Value0),
            1 => Ok(Self::Value1),
            2 => Ok(Self::Value2),
            3 => Ok(Self::Value3),
            4 => Ok(Self::Value4),
            5 => Ok(Self::Value5),
            6 => Ok(Self::Value6),
            7 => Ok(Self::Value7),
            _ => Err(Gdl90Error::InvalidField {
                field: "NEXRAD intensity",
                details: format!("{value} is outside 0..=7"),
            }),
        }
    }

    pub fn encoded_value(self) -> u8 {
        match self {
            Self::Value0 => 0,
            Self::Value1 => 1,
            Self::Value2 => 2,
            Self::Value3 => 3,
            Self::Value4 => 4,
            Self::Value5 => 5,
            Self::Value6 => 6,
            Self::Value7 => 7,
        }
    }

    pub fn reflectivity_range(self) -> &'static str {
        match self {
            Self::Value0 => "dBz < 5",
            Self::Value1 => "5 <= dBz <= 20",
            Self::Value2 => "20 <= dBz <= 30",
            Self::Value3 => "30 <= dBz <= 40",
            Self::Value4 => "40 <= dBz <= 45",
            Self::Value5 => "45 <= dBz <= 50",
            Self::Value6 => "50 <= dBz <= 55",
            Self::Value7 => "55 <= dBz",
        }
    }

    pub fn weather_condition(self) -> &'static str {
        match self {
            Self::Value0 | Self::Value1 => "Background",
            Self::Value2 => "VIP 1",
            Self::Value3 => "VIP 2",
            Self::Value4 => "VIP 3",
            Self::Value5 => "VIP 4",
            Self::Value6 => "VIP 5",
            Self::Value7 => "VIP 6",
        }
    }

    pub fn is_background(self) -> bool {
        matches!(self, Self::Value0 | Self::Value1)
    }
}

impl NexradBlockReference {
    pub fn from_raw(raw: [u8; 3]) -> Self {
        Self {
            is_run_length_encoded: (raw[0] & 0x80) != 0,
            north: (raw[0] & 0x40) != 0,
            scale: (raw[0] >> 4) & 0x03,
            block_number: (((raw[0] & 0x0F) as u32) << 16) | ((raw[1] as u32) << 8) | raw[2] as u32,
        }
    }

    pub fn to_raw(self) -> [u8; 3] {
        let mut first = ((self.scale & 0x03) << 4) | (((self.block_number >> 16) as u8) & 0x0F);
        if self.is_run_length_encoded {
            first |= 0x80;
        }
        if self.north {
            first |= 0x40;
        }
        [
            first,
            ((self.block_number >> 8) & 0xFF) as u8,
            (self.block_number & 0xFF) as u8,
        ]
    }
}

fn encode_apdu_time_option(has_month_day: bool, has_seconds: bool) -> Result<u8> {
    match (has_month_day, has_seconds) {
        (false, false) => Ok(0),
        (false, true) => Ok(APDU_TIME_FLAG_SECONDS),
        (true, false) => Ok(APDU_TIME_FLAG_MONTH_DAY),
        (true, true) => Err(Gdl90Error::InvalidField {
            field: "APDU time flags",
            details: "time flag #1 and time flag #2 cannot both be set".to_string(),
        }),
    }
}

struct BitReader<'a> {
    bytes: &'a [u8],
    bit_offset: usize,
}

impl<'a> BitReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            bit_offset: 0,
        }
    }

    fn read_bool(&mut self) -> Result<bool> {
        Ok(self.read_u8(1)? != 0)
    }

    fn read_u8(&mut self, bits: usize) -> Result<u8> {
        let value = self.read_bits(bits)?;
        u8::try_from(value).map_err(|_| Gdl90Error::InvalidField {
            field: "bit reader",
            details: format!("{value} does not fit in u8"),
        })
    }

    fn read_u16(&mut self, bits: usize) -> Result<u16> {
        let value = self.read_bits(bits)?;
        u16::try_from(value).map_err(|_| Gdl90Error::InvalidField {
            field: "bit reader",
            details: format!("{value} does not fit in u16"),
        })
    }

    fn read_bits(&mut self, bits: usize) -> Result<u32> {
        let mut value = 0u32;
        for _ in 0..bits {
            let byte_index = self.bit_offset / 8;
            if byte_index >= self.bytes.len() {
                return Err(Gdl90Error::InvalidLength {
                    context: "APDU header",
                    expected: "enough bytes for optional fields",
                    actual: self.bytes.len(),
                });
            }
            let bit_index = 7 - (self.bit_offset % 8);
            let bit = (self.bytes[byte_index] >> bit_index) & 0x01;
            value = (value << 1) | u32::from(bit);
            self.bit_offset += 1;
        }
        Ok(value)
    }

    fn align_to_byte_zero_pad(&mut self) -> Result<()> {
        let remainder = self.bit_offset % 8;
        if remainder == 0 {
            return Ok(());
        }
        for _ in remainder..8 {
            if self.read_bool()? {
                return Err(Gdl90Error::InvalidField {
                    field: "APDU zero pad",
                    details: "expected zero padding bits at end of APDU header".to_string(),
                });
            }
        }
        Ok(())
    }

    fn bytes_consumed(&self) -> usize {
        self.bit_offset.div_ceil(8)
    }
}

struct BitWriter {
    bytes: Vec<u8>,
    bit_offset: usize,
}

impl BitWriter {
    fn new() -> Self {
        Self {
            bytes: Vec::new(),
            bit_offset: 0,
        }
    }

    fn push_bool(&mut self, value: bool) {
        self.push_bits(u32::from(value), 1);
    }

    fn push_u8(&mut self, value: u8, bits: usize) {
        self.push_bits(u32::from(value), bits);
    }

    fn push_u16(&mut self, value: u16, bits: usize) {
        self.push_bits(u32::from(value), bits);
    }

    fn push_bits(&mut self, value: u32, bits: usize) {
        for shift in (0..bits).rev() {
            let bit = ((value >> shift) & 0x01) as u8;
            let byte_index = self.bit_offset / 8;
            if byte_index == self.bytes.len() {
                self.bytes.push(0);
            }
            let bit_index = 7 - (self.bit_offset % 8);
            self.bytes[byte_index] |= bit << bit_index;
            self.bit_offset += 1;
        }
    }

    fn finish_zero_padded(mut self) -> Result<Vec<u8>> {
        while self.bit_offset % 8 != 0 {
            self.push_bool(false);
        }
        Ok(self.bytes)
    }
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
        31 => '|',
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
        '|' => Ok(31),
        '\u{001F}' => Ok(31),
        ' '..='?' => Ok(ch as u8),
        _ => Err(Gdl90Error::UnsupportedCharacter {
            context: "DLAC text",
            ch,
        }),
    }
}
