//! Source CSV columns and the typed crime record.

use crate::CivilDateTime;

/// Column names of the source CSV, in file order.
pub const HEADER: [&str; 22] = [
    "ID",
    "Case Number",
    "Date",
    "Block",
    "IUCR",
    "Primary Type",
    "Description",
    "Location Description",
    "Arrest",
    "Domestic",
    "Beat",
    "District",
    "Ward",
    "Community Area",
    "FBI Code",
    "X Coordinate",
    "Y Coordinate",
    "Year",
    "Updated On",
    "Latitude",
    "Longitude",
    "Location",
];

/// One crime with every stored field typed, in CSV column order.
///
/// `Location` is not stored: it is derived from `latitude` and `longitude` (see [`CrimeRecord::location`]).
/// Codes (`iucr`, `beat`, `district`, `fbi_code`) are text so leading zeros survive (`0314`, `003`, `08B`).
/// `None` means the source field was empty.
#[derive(Debug, Clone, PartialEq)]
pub struct CrimeRecord {
    pub id: i64,
    pub case_number: String,
    pub date: CivilDateTime,
    pub block: String,
    pub iucr: String,
    pub primary_type: String,
    pub description: String,
    pub location_description: Option<String>,
    pub arrest: bool,
    pub domestic: bool,
    pub beat: String,
    pub district: Option<String>,
    pub ward: Option<i64>,
    pub community_area: Option<i64>,
    pub fbi_code: String,
    pub x_coordinate: Option<i64>,
    pub y_coordinate: Option<i64>,
    pub year: i64,
    pub updated_on: CivilDateTime,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
}

impl CrimeRecord {
    /// Rebuilds the source `Location` column, `(latitude, longitude)`, when both coordinates exist.
    ///
    /// `f64` formatting is the shortest text that round-trips, so it matches the published text
    /// whenever the source writes no redundant trailing zeros.
    pub fn location(&self) -> Option<String> {
        let (latitude, longitude) = self.latitude.zip(self.longitude)?;
        Some(format!("({latitude}, {longitude})"))
    }
}
