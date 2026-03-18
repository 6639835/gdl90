use gdl90::foreflight::{
    ForeFlightAhrsMessage, ForeFlightCapabilities, ForeFlightIdMessage, GeometricAltitudeDatum,
    Heading, HeadingType, InternetPolicy,
};
use gdl90::message::Message;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let id = Message::ForeFlightId(ForeFlightIdMessage {
        version: 1,
        device_serial_number: Some(42),
        device_name: "GDL90".to_string(),
        device_long_name: "Rust GDL90 Demo".to_string(),
        capabilities: ForeFlightCapabilities {
            geometric_altitude_datum: GeometricAltitudeDatum::MeanSeaLevel,
            internet_policy: InternetPolicy::Disallowed,
            reserved_bits: 0,
        },
    });

    let ahrs = Message::ForeFlightAhrs(ForeFlightAhrsMessage {
        roll_tenths_degrees: Some(25),
        pitch_tenths_degrees: Some(-10),
        heading: Some(Heading {
            heading_type: HeadingType::Magnetic,
            tenths_degrees: 1_234,
        }),
        indicated_airspeed_knots: Some(105),
        true_airspeed_knots: Some(112),
    });

    for message in [id, ahrs] {
        let encoded = message.encode_frame()?;
        let clear = gdl90::frame::decode_frame(&encoded)?;
        let decoded = Message::decode(&clear)?;
        println!("{decoded:#?}");
    }

    Ok(())
}
