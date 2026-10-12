//! The civil date and time of the CSV, rebuilt here without any code of the generator.
//!
//! The dataset stores `Date` and `Updated On` as seconds since `1970-01-01T00:00:00` of the naive
//! wall clock (parent PRD, decision D3): no time zone and no daylight saving time. This module
//! turns those seconds back into the published text `MM/DD/YYYY hh:mm:ss AM|PM`, parses that text
//! into seconds, and recognises the hour that does not exist in Chicago on the spring-forward
//! Sunday (parent AC12).

const SECONDS_PER_DAY: i64 = 86_400;
const SECONDS_PER_HOUR: i64 = 3_600;

/// The published text of `seconds`: `MM/DD/YYYY hh:mm:ss AM|PM` on a 12-hour clock, where the
/// hour after midnight is `12:xx AM` and the hour after noon is `12:xx PM`.
pub fn format_civil(seconds: i64) -> String {
    let (year, month, day) = date_of_day(seconds.div_euclid(SECONDS_PER_DAY));
    let second_of_day = seconds.rem_euclid(SECONDS_PER_DAY);
    let hour = second_of_day / SECONDS_PER_HOUR;
    let minute = second_of_day % SECONDS_PER_HOUR / 60;
    let second = second_of_day % 60;
    let (clock_hour, meridiem) = match hour {
        0 => (12, "AM"),
        1..=11 => (hour, "AM"),
        12 => (12, "PM"),
        _ => (hour - 12, "PM"),
    };
    format!("{month:02}/{day:02}/{year:04} {clock_hour:02}:{minute:02}:{second:02} {meridiem}")
}

/// The seconds of a published text, or `None` unless `text` is exactly
/// `MM/DD/YYYY hh:mm:ss AM|PM` with a date and a time that exist on the calendar and the clock.
pub fn parse_civil(text: &str) -> Option<i64> {
    let bytes = text.as_bytes();
    let shape_matches = bytes.len() == 22
        && bytes[2] == b'/'
        && bytes[5] == b'/'
        && bytes[10] == b' '
        && bytes[13] == b':'
        && bytes[16] == b':'
        && bytes[19] == b' ';
    if !shape_matches {
        return None;
    }
    let number = |range: std::ops::Range<usize>| -> Option<i64> {
        let digits = &bytes[range];
        digits.iter().all(u8::is_ascii_digit).then(|| {
            digits
                .iter()
                .fold(0, |value, &digit| value * 10 + i64::from(digit - b'0'))
        })
    };
    let (month, day, year) = (number(0..2)?, number(3..5)?, number(6..10)?);
    let (clock_hour, minute, second) = (number(11..13)?, number(14..16)?, number(17..19)?);
    let afternoon = match &bytes[20..22] {
        b"AM" => false,
        b"PM" => true,
        _ => return None,
    };
    let date_exists = year >= 1
        && (1..=12).contains(&month)
        && day >= 1
        && day <= month_lengths(year)[usize::try_from(month - 1).ok()?];
    let time_exists = (1..=12).contains(&clock_hour) && minute < 60 && second < 60;
    if !date_exists || !time_exists {
        return None;
    }
    let hour = clock_hour % 12 + if afternoon { 12 } else { 0 };
    let day_number = days_before_year(year)
        + month_lengths(year)[..usize::try_from(month - 1).ok()?]
            .iter()
            .sum::<i64>()
        + day
        - 1;
    Some(day_number * SECONDS_PER_DAY + hour * SECONDS_PER_HOUR + minute * 60 + second)
}

/// Whether `seconds` falls in the hour that the clocks of Chicago skip when daylight saving time
/// starts: from `02:00:00` to `02:59:59 AM` of the first Sunday of April until 2006, and of the
/// second Sunday of March since 2007 (US rules in force for the years of the source data).
pub fn is_spring_forward_gap(seconds: i64) -> bool {
    let day_number = seconds.div_euclid(SECONDS_PER_DAY);
    if seconds.rem_euclid(SECONDS_PER_DAY) / SECONDS_PER_HOUR != 2 {
        return false;
    }
    let (year, month, day) = date_of_day(day_number);
    let (gap_month, sunday) = if year >= 2007 { (3, 2) } else { (4, 1) };
    // Sundays of the month fall on days 1..=7 for the first one, 8..=14 for the second one.
    let first_day = (sunday - 1) * 7 + 1;
    month == gap_month && (first_day..first_day + 7).contains(&day) && is_sunday(day_number)
}

/// 1970-01-01 was a Thursday.
fn is_sunday(day_number: i64) -> bool {
    (day_number + 4).rem_euclid(7) == 0
}

fn is_leap_year(year: i64) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

fn month_lengths(year: i64) -> [i64; 12] {
    let february = if is_leap_year(year) { 29 } else { 28 };
    [31, february, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
}

/// Days from 1970-01-01 to January 1 of `year` (negative before 1970), Gregorian calendar.
fn days_before_year(year: i64) -> i64 {
    let leap_years_up_to =
        |year: i64| year.div_euclid(4) - year.div_euclid(100) + year.div_euclid(400);
    365 * (year - 1970) + leap_years_up_to(year - 1) - leap_years_up_to(1969)
}

/// Year, month and day of the day `day_number` days after 1970-01-01.
fn date_of_day(day_number: i64) -> (i64, i64, i64) {
    // A first guess within a year or two, then corrected in whole years.
    let mut year = 1970 + day_number.div_euclid(365);
    while days_before_year(year) > day_number {
        year -= 1;
    }
    while days_before_year(year + 1) <= day_number {
        year += 1;
    }
    let mut day_of_year = day_number - days_before_year(year);
    let mut month = 1;
    for length in month_lengths(year) {
        if day_of_year < length {
            break;
        }
        day_of_year -= length;
        month += 1;
    }
    (year, month, day_of_year + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Seconds of the naive clock, computed by hand for the dates below.
    const LEAP_DAY_2024: i64 = 1_709_164_800;
    const MARCH_FIRST_2000: i64 = 951_868_800;
    const MARCH_FIRST_2100: i64 = 4_107_542_400;
    const OCTOBER_3_2026: i64 = 1_790_985_600;

    fn seconds(text: &str) -> i64 {
        parse_civil(text).unwrap_or_else(|| panic!("{text:?} should parse"))
    }

    #[test]
    fn the_twelve_hour_clock_has_midnight_and_noon_at_twelve() {
        let cases = [
            (0, "01/01/1970 12:00:00 AM"),
            (3_600, "01/01/1970 01:00:00 AM"),
            (11 * 3_600 + 59 * 60 + 59, "01/01/1970 11:59:59 AM"),
            (12 * 3_600, "01/01/1970 12:00:00 PM"),
            (13 * 3_600 + 5, "01/01/1970 01:00:05 PM"),
            (86_399, "01/01/1970 11:59:59 PM"),
        ];
        for (value, text) in cases {
            assert_eq!(format_civil(value), text, "{value}");
            assert_eq!(parse_civil(text), Some(value), "{text}");
        }
    }

    #[test]
    fn dates_far_from_the_epoch_and_leap_years_are_exact() {
        let cases = [
            (-1, "12/31/1969 11:59:59 PM"),
            (LEAP_DAY_2024, "02/29/2024 12:00:00 AM"),
            (LEAP_DAY_2024 + 86_400, "03/01/2024 12:00:00 AM"),
            (MARCH_FIRST_2000 - 86_400, "02/29/2000 12:00:00 AM"),
            (MARCH_FIRST_2100 - 86_400, "02/28/2100 12:00:00 AM"),
            (OCTOBER_3_2026, "10/03/2026 12:00:00 AM"),
            (
                OCTOBER_3_2026 + 7 * 86_400 + 15 * 3_600 + 42 * 60 + 19,
                "10/10/2026 03:42:19 PM",
            ),
        ];
        for (value, text) in cases {
            assert_eq!(format_civil(value), text, "{value}");
            assert_eq!(parse_civil(text), Some(value), "{text}");
        }
    }

    #[test]
    fn every_formatted_value_parses_back_to_itself() {
        let first = seconds("01/01/2001 12:00:00 AM");
        let last = seconds("12/31/2027 11:59:59 PM");
        // A prime step visits every hour, minute and second over the years.
        for value in (first..=last).step_by(7_919) {
            assert_eq!(parse_civil(&format_civil(value)), Some(value), "{value}");
        }
    }

    #[test]
    fn only_the_exact_published_shape_parses() {
        let invalid = [
            "",
            "1/03/2026 12:00:00 AM",
            "10/3/2026 12:00:00 AM",
            "10-03-2026 12:00:00 AM",
            "10/03/2026 12:00:00",
            "10/03/2026 12:00:00 am",
            "10/03/2026 12:00:00 XM",
            "10/03/2026 12:00:00 AM ",
            "10/03/2026T12:00:00 AM",
            "13/01/2026 12:00:00 AM",
            "00/01/2026 12:00:00 AM",
            "02/29/2026 12:00:00 AM",
            "02/29/2100 12:00:00 AM",
            "04/31/2026 12:00:00 AM",
            "10/00/2026 12:00:00 AM",
            "10/03/2026 00:00:00 AM",
            "10/03/2026 13:00:00 PM",
            "10/03/2026 12:60:00 AM",
            "10/03/2026 12:00:60 AM",
            "1O/03/2026 12:00:00 AM",
            "+1/03/2026 12:00:00 AM",
        ];
        for text in invalid {
            assert_eq!(parse_civil(text), None, "{text:?}");
        }
    }

    #[test]
    fn the_spring_forward_hour_follows_the_us_rules_of_each_year() {
        let gap = [
            // Since 2007: the second Sunday of March.
            "03/08/2015 02:00:00 AM",
            "03/08/2015 02:30:00 AM",
            "03/08/2015 02:59:59 AM",
            "03/11/2007 02:15:00 AM",
            "03/08/2026 02:01:00 AM",
            // Until 2006: the first Sunday of April.
            "04/02/2006 02:15:00 AM",
            "04/01/2001 02:45:00 AM",
        ];
        for text in gap {
            assert!(is_spring_forward_gap(seconds(text)), "{text}");
        }
        let not_gap = [
            "03/08/2015 01:59:59 AM",
            "03/08/2015 03:00:00 AM",
            "03/08/2015 02:30:00 PM",
            // The first Sunday of March 2015 and the second Sunday of March 2006.
            "03/01/2015 02:30:00 AM",
            "03/12/2006 02:30:00 AM",
            "04/01/2007 02:30:00 AM",
            // The repeated hour in the fall exists.
            "11/01/2015 01:30:00 AM",
            "03/09/2015 02:30:00 AM",
        ];
        for text in not_gap {
            assert!(!is_spring_forward_gap(seconds(text)), "{text}");
        }
    }
}
