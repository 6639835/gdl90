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

pub fn section_support_matrix() -> Vec<SectionSupportEntry> {
    vec![
        SectionSupportEntry {
            section: "1.1",
            title: "Purpose",
            state: SupportState::OutOfScopeBehavior,
            notes: "Informational document scope; no binary protocol behavior to implement.",
        },
        SectionSupportEntry {
            section: "1.2",
            title: "Scope",
            state: SupportState::OutOfScopeBehavior,
            notes: "Informational document scope; no binary protocol behavior to implement.",
        },
        SectionSupportEntry {
            section: "1.3",
            title: "Interface Types",
            state: SupportState::OutOfScopeBehavior,
            notes: "Descriptive overview of installation/interface modes rather than message encoding rules.",
        },
        SectionSupportEntry {
            section: "1.4",
            title: "Disclaimer for Display Vendors",
            state: SupportState::OutOfScopeBehavior,
            notes: "Advisory text, not protocol logic.",
        },
        SectionSupportEntry {
            section: "1.5",
            title: "Disclaimer / Warranty / Liability",
            state: SupportState::OutOfScopeBehavior,
            notes: "Legal text, not protocol logic.",
        },
        SectionSupportEntry {
            section: "1.6",
            title: "Glossary",
            state: SupportState::OutOfScopeBehavior,
            notes: "Reference terminology rather than on-wire behavior.",
        },
        SectionSupportEntry {
            section: "2",
            title: "RS-422 Bus Message Structure",
            state: SupportState::Complete,
            notes: "The documented framing, transport characteristics, and bandwidth behavior are implemented.",
        },
        SectionSupportEntry {
            section: "2.1",
            title: "Physical Interface",
            state: SupportState::Complete,
            notes: "RS-422 serial profile and connector mapping are represented in support.rs.",
        },
        SectionSupportEntry {
            section: "2.2",
            title: "Message Structure Overview",
            state: SupportState::Complete,
            notes: "Framing, escaping, message ID handling, and CRC are implemented.",
        },
        SectionSupportEntry {
            section: "2.3",
            title: "Bandwidth Management",
            state: SupportState::Complete,
            notes: "Byte-budget scheduling and documented output order are implemented.",
        },
        SectionSupportEntry {
            section: "3",
            title: "Message Definitions",
            state: SupportState::Partial,
            notes: "All documented outer message formats are implemented; the remaining gap is the externally-defined inner bit layout of pass-through ADS-B payloads.",
        },
        SectionSupportEntry {
            section: "3.1",
            title: "Heartbeat Message",
            state: SupportState::Complete,
            notes: "Heartbeat encode/decode covers status bytes, 17-bit UTC timestamp, and message counters.",
        },
        SectionSupportEntry {
            section: "3.2",
            title: "Initialization Message",
            state: SupportState::Complete,
            notes: "Initialization encode/decode covers both configuration bytes and documented control bits.",
        },
        SectionSupportEntry {
            section: "3.3",
            title: "Uplink Data Message",
            state: SupportState::Complete,
            notes: "Uplink Data encode/decode covers TOR handling and the full 432-byte uplink payload container.",
        },
        SectionSupportEntry {
            section: "3.4",
            title: "Ownship Report Message",
            state: SupportState::Complete,
            notes: "Ownship report handling is implemented through the shared traffic/ownship target-report codec.",
        },
        SectionSupportEntry {
            section: "3.5",
            title: "Traffic Report",
            state: SupportState::Complete,
            notes: "Traffic report encode/decode covers the documented 27-byte payload format, field ranges, and saturation rules.",
        },
        SectionSupportEntry {
            section: "3.5.1",
            title: "Traffic and Ownship Report Data Format",
            state: SupportState::Complete,
            notes: "Address, position, altitude, misc flags, NIC/NACp, velocity, heading, emitter, call sign, and emergency fields are implemented.",
        },
        SectionSupportEntry {
            section: "3.5.2",
            title: "Traffic Report Example",
            state: SupportState::Complete,
            notes: "The published worked example is covered by protocol tests.",
        },
        SectionSupportEntry {
            section: "3.6",
            title: "Pass-Through Reports Inner Payloads",
            state: SupportState::Partial,
            notes: "Basic and Long payloads are structurally decoded into header, state vector, mode status, and auxiliary state vector segments; full bit-level state/mode field decoding still needs DO-282.",
        },
        SectionSupportEntry {
            section: "3.7",
            title: "Height Above Terrain",
            state: SupportState::Complete,
            notes: "Height Above Terrain encode/decode covers signed 1-foot resolution values and the invalid sentinel.",
        },
        SectionSupportEntry {
            section: "3.8",
            title: "Ownship Geometric Altitude Message",
            state: SupportState::Complete,
            notes: "Ownship geometric altitude encode/decode covers 5-foot signed altitude, vertical warning, VFOM, and compatibility with the supplied ForeFlight sentinel typo.",
        },
        SectionSupportEntry {
            section: "4",
            title: "Uplink Payload Format",
            state: SupportState::Partial,
            notes: "Application data and APDU framing are implemented, but some nested fields are explicitly deferred by the Garmin text to external RTCA/FIS-B references.",
        },
        SectionSupportEntry {
            section: "4.1",
            title: "Uplink Message",
            state: SupportState::Partial,
            notes: "The 432-byte payload container is implemented; only the 8-byte UAT-specific header bit layout remains blocked by the external DO-282 reference.",
        },
        SectionSupportEntry {
            section: "4.1.1",
            title: "UAT-Specific Header",
            state: SupportState::BlockedByExternalSpec,
            notes: "The 8-byte header is preserved raw; bit-field layout is deferred by the Garmin document to DO-282.",
        },
        SectionSupportEntry {
            section: "4.1.2",
            title: "Application Data",
            state: SupportState::Complete,
            notes: "The 424-byte application-data region is preserved and parsed into documented information frames.",
        },
        SectionSupportEntry {
            section: "4.2",
            title: "Information Frames",
            state: SupportState::Complete,
            notes: "I-Frame length parsing, frame typing, and APDU/developmental frame handling are implemented.",
        },
        SectionSupportEntry {
            section: "4.3",
            title: "APDU Header and Payload",
            state: SupportState::Partial,
            notes: "Minimal UAT APDUs plus EASA-backed optional time variants and segmentation metadata are parsed and encoded; optional product-descriptor fields and full linked-product reassembly still need the external RTCA definitions.",
        },
        SectionSupportEntry {
            section: "4.4",
            title: "FIS-B Products",
            state: SupportState::Partial,
            notes: "Generic Text and NEXRAD products are typed; other product IDs are preserved and surfaced as unknown registry products, but not decoded.",
        },
        SectionSupportEntry {
            section: "4.5",
            title: "Future Products",
            state: SupportState::NotImplemented,
            notes: "Future FAA registry products are preserved by product ID, but no product-specific decoding is possible until definitions are provided.",
        },
        SectionSupportEntry {
            section: "5",
            title: "FIS-B Product APDU Definition",
            state: SupportState::Partial,
            notes: "The two product definitions included in the Garmin text are implemented as far as the supplied material permits.",
        },
        SectionSupportEntry {
            section: "5.1",
            title: "NEXRAD Global Block Representation",
            state: SupportState::Partial,
            notes: "Run-length payloads, typed intensity semantics from Table 20, and Garmin-ICD-aligned block-reference element/N-S/scale/block-number parsing are implemented; exact geo semantics remain external-spec dependent.",
        },
        SectionSupportEntry {
            section: "5.2",
            title: "Generic Textual Data Product",
            state: SupportState::Partial,
            notes: "Generic Text records, DLAC packing, record-to-APDU packing, the verified Appendix-K pipe-character correction, and METAR/TAF composition are supported; exact full Appendix K coverage is not guaranteed.",
        },
        SectionSupportEntry {
            section: "6",
            title: "Control Panel Interface",
            state: SupportState::Complete,
            notes: "The RS-232 control-panel serial profile and all documented ASCII control messages are implemented.",
        },
        SectionSupportEntry {
            section: "6.1",
            title: "Control Panel Physical Interface",
            state: SupportState::Complete,
            notes: "RS-232 serial profiles at 1200 and 9600 baud plus the documented DB15/P1 pin mapping are represented in support.rs.",
        },
        SectionSupportEntry {
            section: "6.2",
            title: "Control Messages",
            state: SupportState::Complete,
            notes: "Call Sign, Mode, and VFR Code message encode/decode are implemented.",
        },
        SectionSupportEntry {
            section: "ForeFlight Connectivity",
            title: "ForeFlight Connectivity",
            state: SupportState::Complete,
            notes: "Connectivity helper logic for Heartbeat/Ownship presence and the documented packet-size guard are implemented.",
        },
        SectionSupportEntry {
            section: "ForeFlight Broadcast",
            title: "ForeFlight Broadcast",
            state: SupportState::Complete,
            notes: "UDP discovery JSON parsing, configurable target derivation, and the documented 5-second cadence constant are implemented.",
        },
        SectionSupportEntry {
            section: "ForeFlight Messages",
            title: "ForeFlight Message Set",
            state: SupportState::Complete,
            notes: "The documented supported-message subset and UDP datagram encoding rules are implemented.",
        },
        SectionSupportEntry {
            section: "ForeFlight Heartbeat",
            title: "ForeFlight Heartbeat Message",
            state: SupportState::Complete,
            notes: "Heartbeat is supported through the core GDL90 codec and ForeFlight subset validation.",
        },
        SectionSupportEntry {
            section: "ForeFlight UAT Uplink",
            title: "ForeFlight UAT Uplink",
            state: SupportState::Complete,
            notes: "UAT uplink is supported through the core GDL90 codec and ForeFlight subset validation.",
        },
        SectionSupportEntry {
            section: "ForeFlight Ownship",
            title: "ForeFlight Ownship Report",
            state: SupportState::Complete,
            notes: "Ownship report is supported through the core GDL90 codec and ForeFlight subset validation.",
        },
        SectionSupportEntry {
            section: "ForeFlight Geo Altitude",
            title: "ForeFlight Ownship Geometric Altitude",
            state: SupportState::Complete,
            notes: "Ownship geometric altitude is supported through the core GDL90 codec and the ForeFlight capabilities mask handling.",
        },
        SectionSupportEntry {
            section: "ForeFlight Traffic",
            title: "ForeFlight Traffic Report",
            state: SupportState::Complete,
            notes: "Traffic report is supported through the core GDL90 codec and ForeFlight subset validation.",
        },
        SectionSupportEntry {
            section: "ForeFlight ID",
            title: "ForeFlight ID Message",
            state: SupportState::Complete,
            notes: "ID message encode/decode covers version, serial, names, and capability bits.",
        },
        SectionSupportEntry {
            section: "ForeFlight AHRS",
            title: "ForeFlight AHRS Message",
            state: SupportState::Complete,
            notes: "AHRS encode/decode covers roll, pitch, heading type, IAS, TAS, invalid sentinels, and range validation.",
        },
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
    fn missing_sections_include_external_spec_boundaries() {
        let missing = missing_sections();
        assert!(missing.iter().any(|entry| entry.section == "3.6"));
        assert!(missing.iter().any(|entry| entry.section == "4.1.1"));
        assert!(missing.iter().any(|entry| entry.section == "4.3"));
        assert!(!missing.iter().any(|entry| entry.section == "1.1"));
    }
}
