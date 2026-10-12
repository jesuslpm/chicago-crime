//! Fixtures shared by the integration tests.

// Each test crate uses a different subset of these helpers.
#![allow(dead_code)]

use crime_dataset::{CivilDateTime, CrimeRecord};

/// Parses a datetime the test knows to be valid.
pub fn datetime(text: &str) -> CivilDateTime {
    text.parse()
        .unwrap_or_else(|error| panic!("{text:?} should parse: {error}"))
}

/// The first data row of the reference CSV, fully typed.
pub fn sample_record() -> CrimeRecord {
    CrimeRecord {
        id: 14_348_063,
        case_number: "JK438266".to_owned(),
        date: datetime("10/03/2026 12:00:00 AM"),
        block: "064XX S ELLIS AVE".to_owned(),
        iucr: "1090".to_owned(),
        primary_type: "ARSON".to_owned(),
        description: "ATTEMPT ARSON".to_owned(),
        location_description: Some("VEHICLE NON-COMMERCIAL".to_owned()),
        arrest: false,
        domestic: false,
        beat: "0314".to_owned(),
        district: Some("003".to_owned()),
        ward: Some(20),
        community_area: Some(42),
        fbi_code: "09".to_owned(),
        x_coordinate: Some(1_184_164),
        y_coordinate: Some(1_862_455),
        year: 2026,
        updated_on: datetime("10/10/2026 03:42:19 PM"),
        latitude: Some(41.777750747),
        longitude: Some(-87.600396201),
    }
}

/// A record with only the required fields; every optional field is empty.
pub fn sparse_record(id: i64) -> CrimeRecord {
    CrimeRecord {
        id,
        case_number: "HZ100001".to_owned(),
        date: datetime("03/09/2025 02:30:00 AM"),
        block: "001XX N STATE ST".to_owned(),
        iucr: "0110".to_owned(),
        primary_type: "HOMICIDE".to_owned(),
        description: "FIRST DEGREE MURDER".to_owned(),
        location_description: None,
        arrest: true,
        domestic: true,
        beat: "0111".to_owned(),
        district: None,
        ward: None,
        community_area: None,
        fbi_code: "08B".to_owned(),
        x_coordinate: None,
        y_coordinate: None,
        year: 2025,
        updated_on: datetime("01/01/2026 12:00:00 PM"),
        latitude: None,
        longitude: None,
    }
}
