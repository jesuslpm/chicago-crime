//! Naive civil date and time as published by the source (decision D3).

use std::fmt;
use std::str::FromStr;

const SECONDS_PER_DAY: i64 = 86_400;
const MIN_YEAR: i64 = 1;
const MAX_YEAR: i64 = 9_999;

/// Civil local date and time exactly as published, with no time zone and no UTC conversion.
///
/// The inner value counts seconds since `1970-01-01T00:00:00` of the same naive wall clock, so
/// ordering values is chronological ordering of the published text. Wall-clock times that do not
/// exist or repeat in Chicago because of daylight saving time are valid values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CivilDateTime(i64);

/// Why a civil date and time could not be built.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateTimeError {
    /// The text does not have the shape `MM/DD/YYYY hh:mm:ss AM|PM`.
    Format,
    /// A component is out of range (month 13, February 30, minute 60, ...) or the year is
    /// outside `0001..=9999`.
    OutOfRange,
}

impl CivilDateTime {
    /// Builds a value from seconds since `1970-01-01T00:00:00`.
    ///
    /// Fails when the value falls outside the years `0001..=9999`.
    pub fn from_seconds(seconds: i64) -> Result<Self, DateTimeError> {
        let first = days_from_civil(MIN_YEAR, 1, 1) * SECONDS_PER_DAY;
        let last = (days_from_civil(MAX_YEAR, 12, 31) + 1) * SECONDS_PER_DAY - 1;
        if (first..=last).contains(&seconds) {
            Ok(Self(seconds))
        } else {
            Err(DateTimeError::OutOfRange)
        }
    }

    /// Seconds since `1970-01-01T00:00:00` of the naive civil clock.
    pub fn seconds(self) -> i64 {
        self.0
    }
}

impl FromStr for CivilDateTime {
    type Err = DateTimeError;

    /// Parses exactly `MM/DD/YYYY hh:mm:ss AM|PM` (12-hour clock: `12:xx AM` is the midnight hour).
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        // Byte offsets of the fixed shape: MM/DD/YYYY hh:mm:ss AM
        //                                  0  3  6    11 14 17 20
        let bytes = text.as_bytes();
        let separators_match = bytes.len() == 22
            && [
                (2, b'/'),
                (5, b'/'),
                (10, b' '),
                (13, b':'),
                (16, b':'),
                (19, b' '),
            ]
            .iter()
            .all(|&(position, separator)| bytes[position] == separator);
        if !separators_match {
            return Err(DateTimeError::Format);
        }
        let number = |start: usize, len: usize| digits(&bytes[start..start + len]);
        let (month, day, year) = (number(0, 2)?, number(3, 2)?, number(6, 4)?);
        let (hour, minute, second) = (number(11, 2)?, number(14, 2)?, number(17, 2)?);
        let afternoon = match &bytes[20..] {
            b"AM" => false,
            b"PM" => true,
            _ => return Err(DateTimeError::Format),
        };

        let date_exists = (MIN_YEAR..=MAX_YEAR).contains(&year)
            && (1..=12).contains(&month)
            && (1..=days_in_month(year, month)).contains(&day);
        let time_exists = (1..=12).contains(&hour) && minute <= 59 && second <= 59;
        if !date_exists || !time_exists {
            return Err(DateTimeError::OutOfRange);
        }

        let hour_of_day = hour % 12 + if afternoon { 12 } else { 0 };
        let seconds_of_day = hour_of_day * 3_600 + minute * 60 + second;
        Self::from_seconds(days_from_civil(year, month, day) * SECONDS_PER_DAY + seconds_of_day)
    }
}

impl fmt::Display for CivilDateTime {
    /// Formats as `MM/DD/YYYY hh:mm:ss AM|PM`, the same text the source publishes.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let days = self.0.div_euclid(SECONDS_PER_DAY);
        let seconds_of_day = self.0.rem_euclid(SECONDS_PER_DAY);
        let (year, month, day) = civil_from_days(days);
        let hour_of_day = seconds_of_day / 3_600;
        let minute = seconds_of_day % 3_600 / 60;
        let second = seconds_of_day % 60;
        let hour = match hour_of_day % 12 {
            0 => 12,
            hour => hour,
        };
        let meridiem = if hour_of_day < 12 { "AM" } else { "PM" };
        write!(
            f,
            "{month:02}/{day:02}/{year:04} {hour:02}:{minute:02}:{second:02} {meridiem}"
        )
    }
}

impl fmt::Display for DateTimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DateTimeError::Format => f.write_str("expected the format MM/DD/YYYY hh:mm:ss AM|PM"),
            DateTimeError::OutOfRange => f.write_str("the date or time does not exist"),
        }
    }
}

impl std::error::Error for DateTimeError {}

/// Parses a run of ASCII digits; anything else is a format error.
fn digits(bytes: &[u8]) -> Result<i64, DateTimeError> {
    bytes.iter().try_fold(0, |value, &byte| {
        if byte.is_ascii_digit() {
            Ok(value * 10 + i64::from(byte - b'0'))
        } else {
            Err(DateTimeError::Format)
        }
    })
}

fn days_in_month(year: i64, month: i64) -> i64 {
    match month {
        2 if is_leap_year(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

fn is_leap_year(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

/// Days since 1970-01-01 of a proleptic Gregorian date (Howard Hinnant's `days_from_civil`).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let day_of_year = (153 * ((month + 9) % 12) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// Proleptic Gregorian date of a day count since 1970-01-01 (Howard Hinnant's `civil_from_days`).
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let days = days + 719_468;
    let era = days.div_euclid(146_097);
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}
