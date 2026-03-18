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

pub fn section_support_matrix() -> Vec<SectionSupportEntry> {
    vec![
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
            section: "3.1-3.8",
            title: "Standard Message Definitions",
            state: SupportState::Partial,
            notes: "All outer message formats are implemented; section 3.6 inner payload structures remain blocked by DO-282.",
        },
        SectionSupportEntry {
            section: "3.6",
            title: "Pass-Through Reports Inner Payloads",
            state: SupportState::BlockedByExternalSpec,
            notes: "Basic and Long payload internals are defined in RTCA/DO-282 Section 2.2, not in the supplied Garmin text.",
        },
        SectionSupportEntry {
            section: "4.1.1",
            title: "UAT-Specific Header",
            state: SupportState::BlockedByExternalSpec,
            notes: "The 8-byte header is preserved raw; bit-field layout is deferred by the Garmin document to DO-282.",
        },
        SectionSupportEntry {
            section: "4.1.2-4.2",
            title: "Application Data and I-Frames",
            state: SupportState::Complete,
            notes: "Application data, frame extraction, and frame typing are implemented.",
        },
        SectionSupportEntry {
            section: "4.3",
            title: "APDU Header and Payload",
            state: SupportState::Partial,
            notes: "Minimal 32-bit APDU headers and independent APDUs are supported; optional fields and segmentation remain external-spec dependent.",
        },
        SectionSupportEntry {
            section: "4.4",
            title: "FIS-B Products",
            state: SupportState::Partial,
            notes: "Generic Text and NEXRAD products are typed; other registry products are not implemented.",
        },
        SectionSupportEntry {
            section: "4.5",
            title: "Future Products",
            state: SupportState::NotImplemented,
            notes: "Future FAA registry products are unknown until product definitions are provided.",
        },
        SectionSupportEntry {
            section: "5.1",
            title: "NEXRAD Global Block Representation",
            state: SupportState::Partial,
            notes: "Run-length payloads and sample fields decode; exact geo block-reference semantics remain external-spec dependent.",
        },
        SectionSupportEntry {
            section: "5.2",
            title: "Generic Textual Data Product",
            state: SupportState::Partial,
            notes: "Generic Text records, DLAC packing, and METAR/TAF composition are supported; exact full Appendix K DLAC coverage is not guaranteed.",
        },
        SectionSupportEntry {
            section: "6.1",
            title: "Control Panel Physical Interface",
            state: SupportState::Complete,
            notes: "RS-232 serial profiles at 1200 and 9600 baud are represented in support.rs.",
        },
        SectionSupportEntry {
            section: "6.2",
            title: "Control Messages",
            state: SupportState::Complete,
            notes: "Call Sign, Mode, and VFR Code message encode/decode are implemented.",
        },
        SectionSupportEntry {
            section: "ForeFlight Extension",
            title: "ForeFlight GDL90 Extension",
            state: SupportState::Complete,
            notes: "The documented message subset, connectivity-message requirement, MTU guard, discovery flow, ID, and AHRS support are implemented.",
        },
    ]
}

pub fn missing_sections() -> Vec<SectionSupportEntry> {
    section_support_matrix()
        .into_iter()
        .filter(|entry| !matches!(entry.state, SupportState::Complete))
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
    }

    #[test]
    fn missing_sections_include_external_spec_boundaries() {
        let missing = missing_sections();
        assert!(missing.iter().any(|entry| entry.section == "3.6"));
        assert!(missing.iter().any(|entry| entry.section == "4.1.1"));
        assert!(missing.iter().any(|entry| entry.section == "4.3"));
    }
}
