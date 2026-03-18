use gdl90::message::{
    Heartbeat, HeartbeatStatus, Message, OwnshipGeometricAltitude, VerticalFigureOfMerit,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let heartbeat = Message::Heartbeat(Heartbeat {
        status: HeartbeatStatus {
            gps_position_valid: true,
            maintenance_required: false,
            ident: false,
            address_type_talkback: false,
            gps_battery_low: false,
            ratcs: false,
            uat_initialized: true,
            csa_requested: false,
            csa_not_available: false,
            utc_ok: true,
        },
        timestamp_seconds_since_midnight: 12_345,
        uplink_count: 2,
        basic_and_long_count: 17,
    });

    let geo_alt = Message::OwnshipGeometricAltitude(OwnshipGeometricAltitude {
        altitude_feet: 3_500,
        vertical_warning: false,
        vertical_figure_of_merit: VerticalFigureOfMerit::Meters(8),
    });

    for message in [heartbeat, geo_alt] {
        let encoded = message.encode_frame()?;
        let clear = gdl90::frame::decode_frame(&encoded)?;
        let decoded = Message::decode(&clear)?;
        println!("{decoded:#?}");
    }

    Ok(())
}
