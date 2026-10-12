//! Row validation: one CSV row becomes a typed [`CrimeRecord`] or a [`Rejection`], following the
//! field rules of the parent PRD. A rejection never stops the run (decision D5).

use crime_dataset::{CivilDateTime, CrimeRecord, HEADER};
use csv::ByteRecord;

use crate::messages;

/// Column positions in [`HEADER`]. `Location` (21) is derived data and is never read.
mod column {
    pub const ID: usize = 0;
    pub const CASE_NUMBER: usize = 1;
    pub const DATE: usize = 2;
    pub const BLOCK: usize = 3;
    pub const IUCR: usize = 4;
    pub const PRIMARY_TYPE: usize = 5;
    pub const DESCRIPTION: usize = 6;
    pub const LOCATION_DESCRIPTION: usize = 7;
    pub const ARREST: usize = 8;
    pub const DOMESTIC: usize = 9;
    pub const BEAT: usize = 10;
    pub const DISTRICT: usize = 11;
    pub const WARD: usize = 12;
    pub const COMMUNITY_AREA: usize = 13;
    pub const FBI_CODE: usize = 14;
    pub const X_COORDINATE: usize = 15;
    pub const Y_COORDINATE: usize = 16;
    pub const YEAR: usize = 17;
    pub const UPDATED_ON: usize = 18;
    pub const LATITUDE: usize = 19;
    pub const LONGITUDE: usize = 20;
}

/// A row that could not become a record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rejection {
    /// Line of the file where the row starts (the header is line 1).
    pub line: u64,
    pub reason: RejectReason,
    /// The original row re-serialized as one CSV line, for the log.
    pub raw: String,
}

/// The first problem found in a row: column count, then ID, then the other columns left to right.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RejectReason {
    ColumnCount { found: usize },
    InvalidId { value: String },
    MissingRequired { column: &'static str },
    InvalidDate { column: &'static str, value: String },
    InvalidFlag { column: &'static str, value: String },
    InvalidInteger { column: &'static str, value: String },
    InvalidDecimal { column: &'static str, value: String },
    InvalidUtf8 { column: &'static str },
}

impl Rejection {
    /// The column at fault, when the problem belongs to one column.
    pub fn column(&self) -> Option<&'static str> {
        match &self.reason {
            RejectReason::ColumnCount { .. } => None,
            RejectReason::InvalidId { .. } => Some(HEADER[column::ID]),
            RejectReason::MissingRequired { column }
            | RejectReason::InvalidDate { column, .. }
            | RejectReason::InvalidFlag { column, .. }
            | RejectReason::InvalidInteger { column, .. }
            | RejectReason::InvalidDecimal { column, .. }
            | RejectReason::InvalidUtf8 { column } => Some(column),
        }
    }

    /// The exact user-facing message, starting with the row number.
    pub fn message(&self) -> String {
        let line = self.line;
        match &self.reason {
            RejectReason::ColumnCount { found } => {
                messages::wrong_column_count(line, HEADER.len(), *found)
            }
            RejectReason::InvalidId { value } => messages::invalid_id(line, value),
            RejectReason::MissingRequired { column } => messages::missing_required(line, column),
            RejectReason::InvalidDate { column, value } => {
                messages::invalid_date(line, column, value)
            }
            RejectReason::InvalidFlag { column, value } => {
                messages::invalid_flag(line, column, value)
            }
            RejectReason::InvalidInteger { column, value } => {
                messages::invalid_integer(line, column, value)
            }
            RejectReason::InvalidDecimal { column, value } => {
                messages::invalid_decimal(line, column, value)
            }
            RejectReason::InvalidUtf8 { column } => messages::invalid_utf8(line, column),
        }
    }
}

/// Validates the row that starts at `line` of the file.
pub fn validate_row(line: u64, row: &ByteRecord) -> Result<CrimeRecord, Rejection> {
    parse_record(row).map_err(|reason| Rejection {
        line,
        reason,
        raw: to_csv_line(row),
    })
}

fn parse_record(row: &ByteRecord) -> Result<CrimeRecord, RejectReason> {
    use column::*;

    if row.len() != HEADER.len() {
        return Err(RejectReason::ColumnCount { found: row.len() });
    }
    let fields = Fields(row);
    // Struct fields are evaluated in the order they are written, which is column order, so the
    // first problem from the left is the one reported.
    Ok(CrimeRecord {
        id: fields.id()?,
        case_number: fields.required_text(CASE_NUMBER)?,
        date: fields.datetime(DATE)?,
        block: fields.required_text(BLOCK)?,
        iucr: fields.required_text(IUCR)?,
        primary_type: fields.required_text(PRIMARY_TYPE)?,
        description: fields.required_text(DESCRIPTION)?,
        location_description: fields.optional_text(LOCATION_DESCRIPTION)?,
        arrest: fields.flag(ARREST)?,
        domestic: fields.flag(DOMESTIC)?,
        beat: fields.required_text(BEAT)?,
        district: fields.optional_text(DISTRICT)?,
        ward: fields.optional_integer(WARD)?,
        community_area: fields.optional_integer(COMMUNITY_AREA)?,
        fbi_code: fields.required_text(FBI_CODE)?,
        x_coordinate: fields.optional_integer(X_COORDINATE)?,
        y_coordinate: fields.optional_integer(Y_COORDINATE)?,
        year: fields.required_integer(YEAR)?,
        updated_on: fields.datetime(UPDATED_ON)?,
        latitude: fields.optional_decimal(LATITUDE)?,
        longitude: fields.optional_decimal(LONGITUDE)?,
    })
}

/// Typed access to the fields of a row with the expected column count.
struct Fields<'a>(&'a ByteRecord);

impl<'a> Fields<'a> {
    /// The field as text: every stored column must be UTF-8.
    fn text(&self, index: usize) -> Result<&'a str, RejectReason> {
        std::str::from_utf8(&self.0[index]).map_err(|_| RejectReason::InvalidUtf8 {
            column: HEADER[index],
        })
    }

    /// Text of a mandatory column, which must not be empty.
    fn required(&self, index: usize) -> Result<&'a str, RejectReason> {
        match self.text(index)? {
            "" => Err(RejectReason::MissingRequired {
                column: HEADER[index],
            }),
            text => Ok(text),
        }
    }

    /// Text of an optional column: empty means no value, never `Some("")`.
    fn optional(&self, index: usize) -> Result<Option<&'a str>, RejectReason> {
        Ok(Some(self.text(index)?).filter(|text| !text.is_empty()))
    }

    fn id(&self) -> Result<i64, RejectReason> {
        let text = self.text(column::ID)?;
        text.parse().map_err(|_| RejectReason::InvalidId {
            value: text.to_owned(),
        })
    }

    fn required_text(&self, index: usize) -> Result<String, RejectReason> {
        self.required(index).map(str::to_owned)
    }

    fn optional_text(&self, index: usize) -> Result<Option<String>, RejectReason> {
        Ok(self.optional(index)?.map(str::to_owned))
    }

    fn datetime(&self, index: usize) -> Result<CivilDateTime, RejectReason> {
        let text = self.required(index)?;
        text.parse().map_err(|_| RejectReason::InvalidDate {
            column: HEADER[index],
            value: text.to_owned(),
        })
    }

    /// `true` or `false`, exactly.
    fn flag(&self, index: usize) -> Result<bool, RejectReason> {
        match self.required(index)? {
            "true" => Ok(true),
            "false" => Ok(false),
            text => Err(RejectReason::InvalidFlag {
                column: HEADER[index],
                value: text.to_owned(),
            }),
        }
    }

    fn required_integer(&self, index: usize) -> Result<i64, RejectReason> {
        integer(index, self.required(index)?)
    }

    fn optional_integer(&self, index: usize) -> Result<Option<i64>, RejectReason> {
        self.optional(index)?
            .map(|text| integer(index, text))
            .transpose()
    }

    fn optional_decimal(&self, index: usize) -> Result<Option<f64>, RejectReason> {
        self.optional(index)?
            .map(|text| decimal(index, text))
            .transpose()
    }
}

fn integer(index: usize, text: &str) -> Result<i64, RejectReason> {
    text.parse().map_err(|_| RejectReason::InvalidInteger {
        column: HEADER[index],
        value: text.to_owned(),
    })
}

/// A finite decimal: `NaN` and infinities parse as `f64` but are not coordinates.
fn decimal(index: usize, text: &str) -> Result<f64, RejectReason> {
    text.parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
        .ok_or_else(|| RejectReason::InvalidDecimal {
            column: HEADER[index],
            value: text.to_owned(),
        })
}

/// Re-serializes a row as one CSV line, quoting only where needed. Invalid UTF-8 is replaced so
/// the line can be written to the text log.
fn to_csv_line(row: &ByteRecord) -> String {
    let mut writer = csv::Writer::from_writer(Vec::new());
    writer
        .write_byte_record(row)
        .expect("writing CSV to memory cannot fail");
    let bytes = writer
        .into_inner()
        .expect("flushing CSV to memory cannot fail");
    String::from_utf8_lossy(&bytes)
        .trim_end_matches(['\r', '\n'])
        .to_owned()
}
