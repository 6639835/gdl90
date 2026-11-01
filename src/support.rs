use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum SignalType {
    Rs422,
    Rs232,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Parity {
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum FlowControl {
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct SerialProfile {
    pub signal_type: SignalType,
    pub baud_rate: u32,
    pub start_bits: u8,
    pub data_bits: u8,
    pub stop_bits: u8,
    pub parity: Parity,
    pub flow_control: FlowControl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct InterfaceConnection {
    pub signal_name: &'static str,
    pub direction: &'static str,
    pub connector_pin: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum SupportState {
    Complete,
    Partial,
    NotImplemented,
    BlockedByExternalSpec,
    OutOfScopeBehavior,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SectionSupportEntry {
    pub section: &'static str,
    pub title: &'static str,
    pub state: SupportState,
    pub notes: &'static str,
}

pub fn rs422_bus_profile() -> SerialProfile {
    SerialProfile {
        signal_type: SignalType::Rs422,
        baud_rate: 38_400,
        start_bits: 1,
        data_bits: 8,
        stop_bits: 1,
        parity: Parity::None,
        flow_control: FlowControl::None,
    }
}

pub fn rs422_connections() -> [InterfaceConnection; 4] {
    [
        InterfaceConnection {
            signal_name: "Tx-A",
            direction: "Out of GDL 90",
            connector_pin: "J2 - Pin 11",
        },
        InterfaceConnection {
            signal_name: "Tx-B",
            direction: "Out of GDL 90",
            connector_pin: "J2 - Pin 29",
        },
        InterfaceConnection {
            signal_name: "Rx-A",
            direction: "Into GDL 90",
            connector_pin: "J2 - Pin 10",
        },
        InterfaceConnection {
            signal_name: "Rx-B",
            direction: "Into GDL 90",
            connector_pin: "J2 - Pin 28",
        },
    ]
}

pub fn control_panel_profiles() -> [SerialProfile; 2] {
    [
        SerialProfile {
            signal_type: SignalType::Rs232,
            baud_rate: 1_200,
            start_bits: 1,
            data_bits: 8,
            stop_bits: 1,
            parity: Parity::None,
            flow_control: FlowControl::None,
        },
        SerialProfile {
            signal_type: SignalType::Rs232,
            baud_rate: 9_600,
            start_bits: 1,
            data_bits: 8,
            stop_bits: 1,
            parity: Parity::None,
            flow_control: FlowControl::None,
        },
    ]
}

pub fn control_panel_connections() -> [InterfaceConnection; 2] {
    [
        InterfaceConnection {
            signal_name: "Control Rx",
            direction: "Into GDL 90",
            connector_pin: "DB15/P1 - Pin 12",
        },
        InterfaceConnection {
            signal_name: "Ground",
            direction: "Reference",
            connector_pin: "DB15/P1 - Pin 5",
        },
    ]
}

fn support_entry(
    section: &'static str,
    title: &'static str,
    state: SupportState,
    notes: &'static str,
) -> SectionSupportEntry {
    SectionSupportEntry {
        section,
        title,
        state,
        notes,
    }
}

pub fn section_support_matrix() -> Vec<SectionSupportEntry> {
    vec![
        support_entry(
            "1",
            "Introduction",
            SupportState::OutOfScopeBehavior,
            "Document-introduction material; no on-wire protocol behavior to implement.",
        ),
        support_entry(
            "1.1",
            "Purpose",
            SupportState::OutOfScopeBehavior,
            "Informational scope text; no binary protocol behavior to implement.",
        ),
        support_entry(
            "1.2",
            "Scope",
            SupportState::OutOfScopeBehavior,
            "Informational scope text; no binary protocol behavior to implement.",
        ),
        support_entry(
            "1.3",
            "Interface Types",
            SupportState::OutOfScopeBehavior,
            "Descriptive overview of installation/interface modes rather than message encoding rules.",
        ),
        support_entry(
            "1.4",
            "Disclaimer for Display Vendors",
            SupportState::OutOfScopeBehavior,
            "Advisory text, not protocol logic.",
        ),
        support_entry(
            "1.5",
            "Disclaimer; No Warranty; Limitation of Liability",
            SupportState::OutOfScopeBehavior,
            "Legal text, not protocol logic.",
        ),
        support_entry(
            "1.6",
            "Glossary",
            SupportState::OutOfScopeBehavior,
            "Reference terminology rather than on-wire behavior.",
        ),
        support_entry(
            "2",
            "RS-422 Bus Message Structure",
            SupportState::Partial,
            "Framing and bounded scheduling helpers are implemented; hardware RS-422 I/O and installation certification are not part of this crate.",
        ),
        support_entry(
            "2.1",
            "Physical Interface",
            SupportState::Partial,
            "Electrical profile and connector metadata are represented; no hardware driver, wiring validation, or certified installation support is provided.",
        ),
        support_entry(
            "2.2",
            "Message Structure Overview",
            SupportState::Complete,
            "Framing, escaping, message IDs, CRC/FCS, and example-level behavior are implemented.",
        ),
        support_entry(
            "2.2.1",
            "Datalink Structure and Processing",
            SupportState::Complete,
            "HDLC flag handling, byte stuffing, frame extraction, and clear-message recovery are implemented.",
        ),
        support_entry(
            "2.2.2",
            "Message ID",
            SupportState::Complete,
            "Standard message IDs, ForeFlight extension IDs, and unknown-message preservation are implemented.",
        ),
        support_entry(
            "2.2.3",
            "FCS Calculation",
            SupportState::Complete,
            "CRC-CCITT/FCS generation and validation are implemented.",
        ),
        support_entry(
            "2.2.4",
            "Message Example",
            SupportState::Complete,
            "Published framing examples are covered by decode/re-encode tests.",
        ),
        support_entry(
            "2.3",
            "Bandwidth Management",
            SupportState::Partial,
            "A validated one-second selection algorithm is implemented; the application must supply a monotonic transmission loop and backpressure policy.",
        ),
        support_entry(
            "3",
            "Message Definitions",
            SupportState::Partial,
            "Garmin outer message formats are implemented and pass-through payload types are validated; full inner UAT conformance depends on RTCA/DO-282 material outside the public ICD.",
        ),
        support_entry(
            "3.1",
            "Heartbeat Message",
            SupportState::Complete,
            "Heartbeat encode/decode covers status bytes, the mandatory UAT-initialized bit, 17-bit UTC timestamp, and message counters.",
        ),
        support_entry(
            "3.1.1",
            "Status Byte 1",
            SupportState::Complete,
            "All documented status-byte-1 bits are encoded and decoded.",
        ),
        support_entry(
            "3.1.2",
            "Status Byte 2",
            SupportState::Complete,
            "All documented status-byte-2 bits, including timestamp bit 16 and UTC/CSA flags, are encoded and decoded.",
        ),
        support_entry(
            "3.1.3",
            "UAT Time Stamp",
            SupportState::Complete,
            "The 17-bit UTC-seconds timestamp layout is implemented.",
        ),
        support_entry(
            "3.1.4",
            "Received Message Counts",
            SupportState::Complete,
            "Uplink and Basic/Long message counters are encoded and decoded with the documented bit packing, limits, and Basic/Long saturation behavior.",
        ),
        support_entry(
            "3.2",
            "Initialization Message",
            SupportState::Complete,
            "Initialization encode/decode covers both configuration bytes and documented control bits.",
        ),
        support_entry(
            "3.2.1",
            "Configuration Byte 1",
            SupportState::Complete,
            "Audio Test, Audio Inhibit, and CDTI OK bits are implemented.",
        ),
        support_entry(
            "3.2.2",
            "Configuration Byte 2",
            SupportState::Complete,
            "CSA Audio Disable and CSA Disable bits are implemented.",
        ),
        support_entry(
            "3.3",
            "Uplink Data Message",
            SupportState::Complete,
            "Uplink Data encode/decode covers TOR handling, the valid-application-data requirement, and the full 432-byte uplink payload container.",
        ),
        support_entry(
            "3.3.1",
            "Time of Reception (TOR)",
            SupportState::Complete,
            "The 24-bit little-endian TOR field and invalid sentinel are implemented.",
        ),
        support_entry(
            "3.3.2",
            "Uplink Payload",
            SupportState::Complete,
            "The full uplink payload container is preserved and parsed into the documented header/application-data split.",
        ),
        support_entry(
            "3.4",
            "Ownship Report Message",
            SupportState::Complete,
            "Ownship report handling is implemented through the shared traffic/ownship target-report codec.",
        ),
        support_entry(
            "3.5",
            "Traffic Report",
            SupportState::Complete,
            "Traffic report encode/decode covers the documented 27-byte payload format, field ranges, and saturation rules.",
        ),
        support_entry(
            "3.5.1",
            "Traffic and Ownship Report Data Format",
            SupportState::Complete,
            "Address, position, altitude, misc flags, NIC/NACp, velocity, heading, emitter, call sign, and emergency fields are implemented.",
        ),
        support_entry(
            "3.5.2",
            "Traffic Report Example",
            SupportState::Complete,
            "The published worked example is covered by protocol tests.",
        ),
        support_entry(
            "3.6",
            "Pass-Through Reports",
            SupportState::BlockedByExternalSpec,
            "Outer lengths and matching Basic/Long payload types are validated without panic; complete inner-field conformance requires licensed RTCA/DO-282 requirements and vectors.",
        ),
        support_entry(
            "3.7",
            "Height Above Terrain",
            SupportState::Complete,
            "Height Above Terrain encode/decode covers signed 1-foot resolution values and the invalid sentinel.",
        ),
        support_entry(
            "3.8",
            "Ownship Geometric Altitude Message",
            SupportState::Complete,
            "Ownship geometric altitude encode/decode covers 5-foot signed altitude, vertical warning, and the Rev A VFOM sentinels.",
        ),
        support_entry(
            "4",
            "Uplink Payload Format",
            SupportState::Partial,
            "The fixed container, zero-fill, I-Frames, minimal APDUs, and bounded reassembly are implemented; optional descriptors and externally defined products are preserved losslessly when not semantically decoded.",
        ),
        support_entry(
            "4.1",
            "Uplink Message",
            SupportState::Complete,
            "The 432-byte payload container, 8-byte UAT-specific header, and 424-byte application-data region are all decoded into structured fields.",
        ),
        support_entry(
            "4.1.1",
            "UAT-Specific Header",
            SupportState::Complete,
            "Header latitude/longitude, position-valid flag, UTC-coupled flag, application-data-valid flag, slot id, and TIS-B site id are decoded and re-encoded.",
        ),
        support_entry(
            "4.1.2",
            "Application Data",
            SupportState::Complete,
            "The 424-byte application-data region is preserved and parsed into documented information frames.",
        ),
        support_entry(
            "4.2",
            "Information Frames",
            SupportState::Complete,
            "I-Frame length parsing, reserved bits, frame typing, and frame-data extraction are implemented.",
        ),
        support_entry(
            "4.2.1",
            "Length Field",
            SupportState::Complete,
            "I-Frame length encoding and decoding are implemented.",
        ),
        support_entry(
            "4.2.2",
            "Reserved Field",
            SupportState::Complete,
            "The reserved bits are exposed for diagnostics and validated as zero for received and transmitted Rev A information frames.",
        ),
        support_entry(
            "4.2.3",
            "Frame Type Field",
            SupportState::Complete,
            "Frame-type parsing and encoding follows Rev A Table 18: FIS-B APDU, developmental, and reserved frame types.",
        ),
        support_entry(
            "4.2.4",
            "Frame Data Field",
            SupportState::Complete,
            "Frame data is exposed as APDU payloads or raw developmental frame data; Rev A reserved frame types are rejected.",
        ),
        support_entry(
            "4.3",
            "FIS-B Product Encoding (APDUs)",
            SupportState::Partial,
            "Minimal APDU headers and bounded source-scoped reassembly are implemented. Optional Product Descriptor forms are retained as opaque bytes pending the external normative schema.",
        ),
        support_entry(
            "4.3.1",
            "APDU Header",
            SupportState::BlockedByExternalSpec,
            "The public minimal header is decoded semantically. Nonzero optional descriptor flags are no longer rejected or discarded and are preserved losslessly until the external descriptor schema is available.",
        ),
        support_entry(
            "4.3.2",
            "APDU Payload",
            SupportState::Partial,
            "Independent payload bytes are preserved and bounded reassembly is implemented; complete product-specific payload semantics depend on external FAA/RTCA definitions.",
        ),
        support_entry(
            "4.4",
            "FIS-B Products",
            SupportState::BlockedByExternalSpec,
            "Generic Text and the public NEXRAD examples are implemented; complete product-registry conformance requires FAA/RTCA schemas not contained in Garmin Rev A.",
        ),
        support_entry(
            "4.4.1",
            "Textual METAR and TAF Products",
            SupportState::BlockedByExternalSpec,
            "The public examples and implemented DLAC codec are tested, but complete product conformance depends on RTCA/DO-267A and the FAA registry.",
        ),
        support_entry(
            "4.4.2",
            "NEXRAD Graphic Product",
            SupportState::BlockedByExternalSpec,
            "The Garmin examples, run-length blocks, and preserved unknown forms are implemented; full Global Block Representation semantics are externally defined.",
        ),
        support_entry(
            "4.5",
            "Future Products",
            SupportState::OutOfScopeBehavior,
            "Rev A delegates future product schemas to external registries. Unknown products retain their exact product id, header, and payload bytes instead of being guessed or discarded.",
        ),
        support_entry(
            "5",
            "FIS-B Product APDU Definition",
            SupportState::BlockedByExternalSpec,
            "The Garmin examples and public fields are implemented, but the ICD delegates normative product details to RTCA/DO-267A and the FAA registry.",
        ),
        support_entry(
            "5.1",
            "Type 4 NEXRAD Precipitation Image – Global Block Representation",
            SupportState::BlockedByExternalSpec,
            "Garmin does not reproduce the complete copyrighted GBR definition; public examples are decoded while unsupported forms remain lossless.",
        ),
        support_entry(
            "5.1.1",
            "Definition",
            SupportState::BlockedByExternalSpec,
            "The complete Global Block Representation definition is outside the Garmin public ICD.",
        ),
        support_entry(
            "5.1.2",
            "Assumptions",
            SupportState::OutOfScopeBehavior,
            "Display-side overlap/merge guidance is advisory behavior rather than on-wire protocol logic.",
        ),
        support_entry(
            "5.1.3",
            "APDU Payload Format",
            SupportState::BlockedByExternalSpec,
            "Public header and example forms are implemented, but the complete payload format is delegated to external specifications.",
        ),
        support_entry(
            "5.1.4",
            "FIS-B Graphical Example",
            SupportState::Complete,
            "The published NEXRAD sample application data fields are covered by protocol tests.",
        ),
        support_entry(
            "5.2",
            "Generic Textual Data Product – Type 2 (DLAC)",
            SupportState::BlockedByExternalSpec,
            "The public examples and implemented record codec are tested; the normative DLAC/product definitions remain external.",
        ),
        support_entry(
            "5.2.1",
            "Definition",
            SupportState::BlockedByExternalSpec,
            "The complete Generic Text Type 2 and DLAC definitions are referenced to external RTCA material.",
        ),
        support_entry(
            "5.2.2",
            "APDU Payload Format",
            SupportState::BlockedByExternalSpec,
            "The implemented payload codec covers public examples, but complete normative character and packing behavior is externally controlled.",
        ),
        support_entry(
            "5.2.3",
            "METAR / TAF Composition",
            SupportState::Partial,
            "The public token structure and examples are validated; the FAA product registry remains authoritative for complete composition rules.",
        ),
        support_entry(
            "5.2.4",
            "FIS-B Text Example",
            SupportState::Complete,
            "The published text sample application data field is covered by protocol tests.",
        ),
        support_entry(
            "6",
            "Control Panel Interface",
            SupportState::Partial,
            "ASCII codecs and deterministic one-second/one-minute cadence scheduling are implemented; physical serial transport and certified equipment integration are not provided.",
        ),
        support_entry(
            "6.1",
            "Physical Interface",
            SupportState::Partial,
            "Baud/framing and pin metadata are represented, but this crate does not open or validate a physical RS-232 device.",
        ),
        support_entry(
            "6.2",
            "Control Messages",
            SupportState::Complete,
            "Call Sign, Mode, and VFR Code message encode/decode are implemented.",
        ),
        support_entry(
            "6.2.1",
            "Call Sign Message",
            SupportState::Complete,
            "Call Sign control-message encode/decode, checksum handling, and fixed-width ASCII formatting are implemented.",
        ),
        support_entry(
            "6.2.2",
            "Mode Message",
            SupportState::Complete,
            "Mode control-message encode/decode covers mode, ident, squawk, emergency code, health bit, and checksum.",
        ),
        support_entry(
            "6.2.3",
            "VFR Code Message",
            SupportState::Complete,
            "VFR Code control-message encode/decode and checksum handling are implemented.",
        ),
        support_entry(
            "ForeFlight Connectivity",
            "ForeFlight Connectivity",
            SupportState::Partial,
            "Connectivity detection, IP-aware packet budgets, and a deterministic cadence scheduler are implemented; an application must drive the live loop and verify device interoperability.",
        ),
        support_entry(
            "ForeFlight Broadcast",
            "ForeFlight Broadcast",
            SupportState::Partial,
            "Discovery parsing, target derivation, and a five-second scheduler are implemented; continuous broadcast lifecycle and network-change handling belong to the embedding application.",
        ),
        support_entry(
            "ForeFlight Messages",
            "ForeFlight Message Set",
            SupportState::Partial,
            "The supported subset and packet rules are implemented, with explicit compatibility handling for published VFOM and heading ambiguities.",
        ),
        support_entry(
            "ForeFlight Heartbeat",
            "ForeFlight Heartbeat Message",
            SupportState::Complete,
            "Heartbeat is supported through the core GDL90 codec and ForeFlight subset validation.",
        ),
        support_entry(
            "ForeFlight UAT Uplink",
            "ForeFlight UAT Uplink",
            SupportState::Complete,
            "UAT uplink is supported through the core GDL90 codec and ForeFlight subset validation.",
        ),
        support_entry(
            "ForeFlight Ownship",
            "ForeFlight Ownship Report",
            SupportState::Complete,
            "Ownship report is supported through the core GDL90 codec and ForeFlight subset validation.",
        ),
        support_entry(
            "ForeFlight Geo Altitude",
            "ForeFlight Ownship Geometric Altitude",
            SupportState::Partial,
            "Garmin and ForeFlight publish conflicting greater-than VFOM sentinels; strict and opt-in compatibility modes are implemented pending device interoperability evidence.",
        ),
        support_entry(
            "ForeFlight Traffic",
            "ForeFlight Traffic Report",
            SupportState::Complete,
            "Traffic report is supported through the core GDL90 codec and ForeFlight subset validation.",
        ),
        support_entry(
            "ForeFlight ID",
            "ForeFlight ID Message",
            SupportState::Complete,
            "ID message encode/decode covers version, serial, names, and capability bits.",
        ),
        support_entry(
            "ForeFlight AHRS",
            "ForeFlight AHRS Message",
            SupportState::Partial,
            "Roll, pitch, heading type, airspeeds, sentinels, and cadence are implemented. Negative heading inputs are angle-canonicalized because the published wire field does not define a signed encoding.",
        ),
    ]
}

pub fn missing_sections() -> Vec<SectionSupportEntry> {
    section_support_matrix()
        .into_iter()
        .filter(|entry| {
            matches!(
                entry.state,
                SupportState::Partial
                    | SupportState::NotImplemented
                    | SupportState::BlockedByExternalSpec
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interface_profiles_match_spec() {
        assert_eq!(rs422_bus_profile().baud_rate, 38_400);
        assert_eq!(control_panel_profiles()[0].baud_rate, 1_200);
        assert_eq!(control_panel_profiles()[1].baud_rate, 9_600);
        assert_eq!(rs422_connections()[0].connector_pin, "J2 - Pin 11");
        assert_eq!(
            control_panel_connections()[0].connector_pin,
            "DB15/P1 - Pin 12"
        );
    }

    #[test]
    fn support_matrix_matches_garmin_toc_in_order() {
        let expected = vec![
            "1", "1.1", "1.2", "1.3", "1.4", "1.5", "1.6", "2", "2.1", "2.2", "2.2.1", "2.2.2",
            "2.2.3", "2.2.4", "2.3", "3", "3.1", "3.1.1", "3.1.2", "3.1.3", "3.1.4", "3.2",
            "3.2.1", "3.2.2", "3.3", "3.3.1", "3.3.2", "3.4", "3.5", "3.5.1", "3.5.2", "3.6",
            "3.7", "3.8", "4", "4.1", "4.1.1", "4.1.2", "4.2", "4.2.1", "4.2.2", "4.2.3", "4.2.4",
            "4.3", "4.3.1", "4.3.2", "4.4", "4.4.1", "4.4.2", "4.5", "5", "5.1", "5.1.1", "5.1.2",
            "5.1.3", "5.1.4", "5.2", "5.2.1", "5.2.2", "5.2.3", "5.2.4", "6", "6.1", "6.2",
            "6.2.1", "6.2.2", "6.2.3",
        ];

        let actual = section_support_matrix()
            .into_iter()
            .filter(|entry| !entry.section.starts_with("ForeFlight"))
            .map(|entry| entry.section)
            .collect::<Vec<_>>();
        assert_eq!(actual, expected);
    }

    #[test]
    fn support_matrix_exposes_known_external_and_transport_boundaries() {
        let missing = missing_sections();
        for section in [
            "2.1",
            "3.6",
            "4.3.1",
            "4.3.2",
            "4.4",
            "4.4.1",
            "4.4.2",
            "5.1",
            "5.2",
            "6.1",
            "ForeFlight Geo Altitude",
            "ForeFlight AHRS",
        ] {
            assert!(missing.iter().any(|entry| entry.section == section));
        }
    }
}
