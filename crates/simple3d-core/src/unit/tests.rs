use super::*;

#[test]
fn an_angle_is_wrapped_into_a_single_turn() {
    // Issue 84: a rotation field reads the direction the body faces.
    assert_eq!(wrap_degrees(0.0), 0.0);
    assert_eq!(wrap_degrees(359.0), 359.0);
    assert_eq!(wrap_degrees(360.0), 0.0);
    assert_eq!(wrap_degrees(361.0), 1.0);
    assert_eq!(wrap_degrees(720.0), 0.0);
    assert_eq!(wrap_degrees(-90.0), 270.0);
    assert_eq!(wrap_degrees(-360.0), 0.0);
    // Never the string "360": a hair short of a full turn is one, because
    // four decimals cannot tell them apart and the range has to hold.
    for deg in [-1e-15, -1e-9, 359.999_99, -0.000_001] {
        let wrapped = wrap_degrees(deg);
        assert!((0.0..360.0).contains(&wrapped), "{deg} wrapped to {wrapped}");
        assert_ne!(format_angle(wrapped), "360", "{deg} still reads as a full turn");
    }
    // Nothing to turn by is nothing turned, rather than a NaN in the scene.
    assert_eq!(wrap_degrees(f64::NAN), 0.0);
    assert_eq!(wrap_degrees(f64::INFINITY), 0.0);
}

#[test]
fn display_unit_round_trips_without_rescaling() {
    // Spec acceptance criterion 6.
    let plate = [40.0, 20.0, 4.0];
    let shown: Vec<String> = plate.iter().map(|&mm| format_length(mm, Unit::Metre)).collect();
    assert_eq!(shown, vec!["0.04", "0.02", "0.004"]);
    for (&mm, s) in plate.iter().zip(shown.iter()) {
        assert_eq!(parse_length(s, Unit::Metre), Some(mm));
    }
    for &mm in &plate {
        assert_eq!(parse_length(&format_length(mm, Unit::Millimetre), Unit::Millimetre), Some(mm));
    }
}

#[test]
fn typing_in_metres_stores_millimetres() {
    assert_eq!(parse_length("1.8", Unit::Metre), Some(1800.0));
    assert_eq!(format_length(1800.0, Unit::Metre), "1.8");
}

#[test]
fn both_decimal_separators_are_accepted() {
    assert_eq!(parse_number("1,8"), Some(1.8));
    assert_eq!(parse_number("1.8"), Some(1.8));
    assert_eq!(parse_number(" 12 mm "), Some(12.0));
    assert_eq!(parse_number("-0,5"), Some(-0.5));
}

#[test]
fn garbage_is_rejected_rather_than_guessed() {
    // Acceptance criterion 14: the caller restores the previous value.
    for bad in ["", "  ", "abc", "1.2.3", "1,2,3", "--4", "NaN", "inf", "12mmm"] {
        assert_eq!(parse_number(bad), None, "{bad:?} should not parse");
    }
}

#[test]
fn a_field_reads_an_expression() {
    // The design's own examples.
    assert_eq!(parse_number("40/3"), Some(40.0 / 3.0));
    assert_eq!(parse_number("12+8"), Some(20.0));
    assert_eq!(parse_number("(2+3)*4"), Some(20.0));
    assert_eq!(parse_number("100 - 2*15"), Some(70.0));
    assert_eq!(parse_number("-3*4"), Some(-12.0));
}

#[test]
fn an_expression_that_does_not_resolve_to_a_number_is_refused() {
    // Division by zero must not leave a field holding infinity: there is no
    // shape that could be drawn from it.
    for bad in ["1/0", "5*", "(1+2", "1+2)", "()", "+", "*3", "2 3"] {
        assert_eq!(parse_number(bad), None, "{bad:?} should not parse");
    }
}

#[test]
fn a_value_may_be_typed_in_another_unit_than_the_document_shows() {
    // "4 cm" in a millimetre document is 40 of what the field shows.
    assert_eq!(parse_entry("4 cm", Unit::Millimetre).unwrap().value, 40.0);
    assert_eq!(parse_entry("4cm", Unit::Millimetre).unwrap().value, 40.0);
    assert_eq!(parse_entry("4 cm", Unit::Metre).unwrap().value, 0.04);
    assert_eq!(parse_entry("1 m", Unit::Centimetre).unwrap().value, 100.0);
    // And it stores the same millimetres either way round.
    assert_eq!(parse_length("4cm", Unit::Millimetre), Some(40.0));
    assert_eq!(parse_length("4cm", Unit::Metre), Some(40.0));
    // A suffix inside an expression converts that term alone.
    assert_eq!(parse_entry("4cm + 5", Unit::Millimetre).unwrap().value, 45.0);
}

#[test]
fn a_leading_plus_is_a_delta_and_a_bare_minus_is_not() {
    // `+2` adjusts; `-5` still has to mean minus five, because a position
    // field must be able to hold one.
    assert_eq!(parse_entry("+2", Unit::Millimetre), Some(Entry { value: 2.0, relative: true }));
    assert_eq!(parse_entry("+= 2", Unit::Millimetre), Some(Entry { value: 2.0, relative: true }));
    assert_eq!(parse_entry("-5", Unit::Millimetre), Some(Entry { value: -5.0, relative: false }));
    assert_eq!(parse_entry("- 5", Unit::Millimetre), Some(Entry { value: -5.0, relative: true }));
    assert_eq!(parse_entry("-= 5", Unit::Millimetre), Some(Entry { value: -5.0, relative: true }));
    // A delta is an expression too, and carries its own unit.
    assert_eq!(parse_entry("+2*3", Unit::Millimetre).unwrap().value, 6.0);
    assert_eq!(parse_entry("+1cm", Unit::Millimetre).unwrap().value, 10.0);
    // An absolute entry refuses to be read as a relative one.
    assert_eq!(parse_length("+2", Unit::Millimetre), None);
}

#[test]
fn a_delta_resolves_against_whatever_the_field_holds() {
    let entry = parse_entry("+2", Unit::Millimetre).unwrap();
    // The same typed text gives a different answer per node, which is what
    // makes it work across a multi-selection.
    assert_eq!(entry.resolve(10.0), 12.0);
    assert_eq!(entry.resolve(40.0), 42.0);
    let absolute = parse_entry("2", Unit::Millimetre).unwrap();
    assert_eq!(absolute.resolve(10.0), 2.0);
    assert_eq!(absolute.resolve(40.0), 2.0);
}

#[test]
fn no_floating_point_noise_in_output() {
    assert_eq!(format_number(0.1 + 0.2, 4), "0.3");
    assert_eq!(format_number(1.7999999999, 4), "1.8");
    assert_eq!(format_number(-0.00001, 4), "0");
    assert_eq!(format_number(5.0, 4), "5");
}
