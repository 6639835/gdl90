from pathlib import Path

path = Path(__file__).resolve().parents[1] / "tests" / "protocol.rs"
content = path.read_text(encoding="utf-8")
old = '''#[test]
fn foreflight_ahrs_heading_rejects_negative_values_on_encode() {
    let error = Message::ForeFlightAhrs(ForeFlightAhrsMessage {
        roll_tenths_degrees: None,
        pitch_tenths_degrees: None,
        heading: Some(Heading {
            heading_type: HeadingType::True,
            tenths_degrees: -1,
        }),
        indicated_airspeed_knots: None,
        true_airspeed_knots: None,
    })
    .encode()
    .unwrap_err();

    assert!(
        matches!(error, gdl90::Gdl90Error::InvalidField { field, .. } if field == "AHRS heading")
    );
}
'''
new = '''#[test]
fn foreflight_ahrs_heading_rejects_only_values_outside_the_published_range() {
    let error = Message::ForeFlightAhrs(ForeFlightAhrsMessage {
        roll_tenths_degrees: None,
        pitch_tenths_degrees: None,
        heading: Some(Heading {
            heading_type: HeadingType::True,
            tenths_degrees: -3601,
        }),
        indicated_airspeed_knots: None,
        true_airspeed_knots: None,
    })
    .encode()
    .unwrap_err();

    assert!(
        matches!(error, gdl90::Gdl90Error::InvalidField { field, .. } if field == "AHRS heading")
    );
}
'''
if content.count(old) != 1:
    raise RuntimeError("expected one legacy AHRS heading test")
path.write_text(content.replace(old, new, 1), encoding="utf-8")
