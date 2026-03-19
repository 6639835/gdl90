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
            SupportState::Complete,
            "The documented framing, transport characteristics, and bandwidth behavior are implemented.",
        ),
        support_entry(
            "2.1",
            "Physical Interface",
            SupportState::Complete,
            "RS-422 serial profile and connector mapping are represented in support.rs.",
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
            SupportState::Complete,
            "Byte-budget scheduling and documented output order are implemented.",
        ),
        support_entry(
            "3",
            "Message Definitions",
            SupportState::Partial,
            "All documented outer message formats are implemented; the remaining gap is the externally-defined inner bit layout of pass-through ADS-B payloads.",
        ),
        support_entry(
            "3.1",
            "Heartbeat Message",
            SupportState::Complete,
            "Heartbeat encode/decode covers status bytes, 17-bit UTC timestamp, and message counters.",
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
            "Uplink and Basic/Long message counters are encoded and decoded with the documented bit packing and limits.",
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
            "Uplink Data encode/decode covers TOR handling and the full 432-byte uplink payload container.",
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
            SupportState::Partial,
            "Basic and Long payloads are structurally decoded into header, state vector, mode status, and auxiliary state vector segments; full bit-level state/mode field decoding still needs DO-282.",
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
            "Ownship geometric altitude encode/decode covers 5-foot signed altitude, vertical warning, VFOM, and compatibility with the supplied ForeFlight sentinel typo.",
        ),
        support_entry(
            "4",
            "Uplink Payload Format",
            SupportState::Partial,
            "Application data and APDU framing are implemented, but some nested fields are explicitly deferred by the Garmin text to external RTCA/FIS-B references.",
        ),
        support_entry(
            "4.1",
            "Uplink Message",
            SupportState::Partial,
            "The 432-byte payload container is implemented; only the 8-byte UAT-specific header bit layout remains blocked by the external DO-282 reference.",
        ),
        support_entry(
            "4.1.1",
            "UAT-Specific Header",
            SupportState::BlockedByExternalSpec,
            "The 8-byte header is preserved raw; bit-field layout is deferred by the Garmin document to DO-282.",
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
            "The reserved field is preserved as structured data on decode and re-encode.",
        ),
        support_entry(
            "4.2.3",
            "Frame Type Field",
            SupportState::Complete,
            "Frame-type parsing and encoding for FIS-B APDUs, developmental frames, and reserved values are implemented.",
        ),
        support_entry(
            "4.2.4",
            "Frame Data Field",
            SupportState::Complete,
            "Frame data is exposed as APDU payloads or raw developmental/reserved frame data, as documented.",
        ),
        support_entry(
            "4.3",
            "FIS-B Product Encoding (APDUs)",
            SupportState::Partial,
            "Minimal UAT APDUs plus optional time variants and segmentation metadata are parsed and encoded; optional product-descriptor fields and full linked-product reassembly still need external RTCA definitions.",
        ),
        support_entry(
            "4.3.1",
            "APDU Header",
            SupportState::Partial,
            "Core APDU headers plus optional time and segmentation fields are implemented, but product-descriptor option fields remain externally specified.",
        ),
        support_entry(
            "4.3.2",
            "APDU Payload",
            SupportState::Partial,
            "Independent APDU payload handling is implemented; full linked/segmented product reassembly still needs the external RTCA definitions.",
        ),
        support_entry(
            "4.4",
            "FIS-B Products",
            SupportState::Partial,
            "Generic Text and NEXRAD products are typed; other product IDs are preserved and surfaced as unknown registry products, but not decoded.",
        ),
        support_entry(
            "4.4.1",
            "Textual METAR and TAF Products",
            SupportState::Partial,
            "Generic Text APDUs, METAR/TAF record parsing, and composition validation are implemented, but full registry-level text coverage remains external-registry dependent.",
        ),
        support_entry(
            "4.4.2",
            "NEXRAD Graphic Product",
            SupportState::Partial,
            "NEXRAD APDUs, run-length blocks, and intensity semantics are implemented, but exact geographic interpretation remains external-spec dependent.",
        ),
        support_entry(
            "4.5",
            "Future Products",
            SupportState::NotImplemented,
            "Future FAA registry products are preserved by product ID, but no product-specific decoding is possible until definitions are provided.",
        ),
        support_entry(
            "5",
            "FIS-B Product APDU Definition",
            SupportState::Partial,
            "The two product definitions included in the Garmin text are implemented as far as the supplied material permits.",
        ),
        support_entry(
            "5.1",
            "Type 4 NEXRAD Precipitation Image – Global Block Representation",
            SupportState::Partial,
            "Run-length payloads, typed intensity semantics from Table 20, and Garmin-ICD-aligned block-reference parsing are implemented; exact geo semantics remain external-spec dependent.",
        ),
        support_entry(
            "5.1.1",
            "Definition",
            SupportState::Partial,
            "The documented NEXRAD block payload form is implemented, but the external Global Block geo definition is only partially available from the supplied docs.",
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
            SupportState::Partial,
            "The documented APDU header constraints, block reference, and run-length payloads are implemented; exact geo semantics remain external-spec dependent.",
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
            SupportState::Partial,
            "Generic Text records, DLAC packing, record-to-APDU packing, the verified Appendix-K pipe-character correction, and METAR/TAF composition are supported; exact full Appendix K coverage is not guaranteed.",
        ),
        support_entry(
            "5.2.1",
            "Definition",
            SupportState::Partial,
            "The Generic Text product model is implemented, but full Appendix-K/registry detail is not fully present in the supplied docs.",
        ),
        support_entry(
            "5.2.2",
            "APDU Payload Format",
            SupportState::Partial,
            "Generic Text APDU payload packing and DLAC encode/decode are implemented, but exact full Appendix K coverage is not guaranteed.",
        ),
        support_entry(
            "5.2.3",
            "METAR / TAF Composition",
            SupportState::Complete,
            "METAR/TAF token structure, qualifiers, NIL handling, and whole-record validation are implemented.",
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
            SupportState::Complete,
            "The RS-232 control-panel serial profile and all documented ASCII control messages are implemented.",
        ),
        support_entry(
            "6.1",
            "Physical Interface",
            SupportState::Complete,
            "RS-232 serial profiles at 1200 and 9600 baud plus the documented DB15/P1 pin mapping are represented in support.rs.",
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
            SupportState::Complete,
            "Connectivity helper logic for Heartbeat/Ownship presence and the documented packet-size guard are implemented.",
        ),
        support_entry(
            "ForeFlight Broadcast",
            "ForeFlight Broadcast",
            SupportState::Complete,
            "UDP discovery JSON parsing, configurable target derivation, and the documented 5-second cadence constant are implemented.",
        ),
        support_entry(
            "ForeFlight Messages",
            "ForeFlight Message Set",
            SupportState::Complete,
            "The documented supported-message subset and UDP datagram encoding rules are implemented.",
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
            SupportState::Complete,
            "Ownship geometric altitude is supported through the core GDL90 codec and the ForeFlight capabilities mask handling.",
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
            SupportState::Complete,
            "AHRS encode/decode covers roll, pitch, heading type, IAS, TAS, invalid sentinels, and range validation.",
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
    fn missing_sections_match_expected_protocol_gap_set() {
        let expected = vec![
            "3", "3.6", "4", "4.1", "4.1.1", "4.3", "4.3.1", "4.3.2", "4.4", "4.4.1", "4.4.2",
            "4.5", "5", "5.1", "5.1.1", "5.1.3", "5.2", "5.2.1", "5.2.2",
        ];

        let actual = missing_sections()
            .into_iter()
            .map(|entry| entry.section)
            .collect::<Vec<_>>();

        assert_eq!(actual, expected);
    }
}
