use crime_dataset::{CivilDateTime, DateTimeError};

fn parse(text: &str) -> CivilDateTime {
    text.parse()
        .unwrap_or_else(|error| panic!("{text:?} should parse: {error}"))
}

#[test]
fn formats_back_to_the_published_text() {
    for text in [
        "10/03/2026 12:00:00 AM",
        "10/10/2026 03:42:19 PM",
        "02/29/2024 11:59:59 PM",
        "01/01/0001 12:00:00 AM",
        "12/31/9999 11:59:59 PM",
    ] {
        assert_eq!(parse(text).to_string(), text);
    }
}

#[test]
fn twelve_hour_clock_puts_midnight_first_and_noon_at_twelve() {
    let midnight = parse("10/03/2026 12:00:00 AM");
    let one_am = parse("10/03/2026 01:00:00 AM");
    let noon = parse("10/03/2026 12:00:00 PM");

    assert!(midnight < one_am);
    assert!(one_am < noon);
    assert_eq!(one_am.seconds() - midnight.seconds(), 3_600);
    assert_eq!(noon.seconds() - midnight.seconds(), 12 * 3_600);
}

#[test]
fn seconds_count_from_the_naive_1970_epoch() {
    assert_eq!(parse("01/01/1970 12:00:00 AM").seconds(), 0);
    assert_eq!(
        parse("01/02/1970 01:00:01 AM").seconds(),
        86_400 + 3_600 + 1
    );
    assert_eq!(parse("12/31/1969 11:59:59 PM").seconds(), -1);
}

#[test]
fn accepts_a_wall_clock_time_skipped_by_daylight_saving() {
    let text = "03/09/2025 02:30:00 AM";
    let skipped = parse(text);

    assert_eq!(skipped.to_string(), text);
    // No time zone is involved: the skipped hour sits between its naive neighbours.
    assert!(parse("03/09/2025 01:59:59 AM") < skipped);
    assert!(skipped < parse("03/09/2025 03:00:00 AM"));
}

#[test]
fn rejects_text_with_another_shape() {
    for text in [
        "10/03/2026 12:00:00",
        "2026-10-03 12:00:00",
        "",
        "10/03/2026 12:00:00 am",
        "1/03/2026 12:00:00 AM",
        "10/03/2026 12:00:00 AM ",
        "10-03-2026 12:00:00 AM",
        "10/03/2026 12:00:+0 AM",
    ] {
        assert_eq!(
            text.parse::<CivilDateTime>(),
            Err(DateTimeError::Format),
            "{text:?}"
        );
    }
}

#[test]
fn rejects_dates_and_times_that_do_not_exist() {
    for text in [
        "13/01/2020 01:00:00 AM",
        "02/30/2020 01:00:00 AM",
        "02/29/2023 01:00:00 AM",
        "00/10/2020 01:00:00 AM",
        "04/31/2020 01:00:00 AM",
        "10/03/2026 00:30:00 AM",
        "10/03/2026 13:00:00 PM",
        "10/03/2026 12:60:00 AM",
        "10/03/2026 12:00:60 AM",
        "01/01/0000 12:00:00 AM",
    ] {
        assert_eq!(
            text.parse::<CivilDateTime>(),
            Err(DateTimeError::OutOfRange),
            "{text:?}"
        );
    }
}

#[test]
fn from_seconds_round_trips_and_rejects_years_outside_the_range() {
    let first = parse("01/01/0001 12:00:00 AM");
    let last = parse("12/31/9999 11:59:59 PM");
    let sample = parse("10/10/2026 03:42:19 PM");

    for value in [first, last, sample] {
        assert_eq!(CivilDateTime::from_seconds(value.seconds()), Ok(value));
    }
    assert_eq!(
        CivilDateTime::from_seconds(first.seconds() - 1),
        Err(DateTimeError::OutOfRange)
    );
    assert_eq!(
        CivilDateTime::from_seconds(last.seconds() + 1),
        Err(DateTimeError::OutOfRange)
    );
    assert_eq!(
        CivilDateTime::from_seconds(i64::MIN),
        Err(DateTimeError::OutOfRange)
    );
}
