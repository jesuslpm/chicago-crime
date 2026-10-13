//! The records of a result in the analyst's format (decision C4), written one at a time as the
//! execution yields them: nothing here holds more than the record being written (decision C7).
//!
//! - **CSV**: the 22 source columns under the source header, written like the source file:
//!   every non-empty field quoted (a `"` inside doubled), every empty field left bare (`,,`) and
//!   every line ended by `\n`, so a record comes out as its source line byte for byte. Text is written as stored (the CSV text as is), dates
//!   in the source text `MM/DD/YYYY hh:mm:ss AM|PM`, booleans `true`/`false`, `Location`
//!   rebuilt from the coordinates and "sin valor" as an empty field.
//! - **JSON**: one object, `{"query": …, "dataset": …, "results": [ … ]}`, with the interpreted
//!   query and the version read (decision C9) and one record per line in `results`. Records are
//!   keyed by the CSV header in CSV order with typed values: numbers, booleans, dates in the
//!   source text and `null` for "sin valor".
//! - **Table**: a header row, a separator row and one row per record with every column. The
//!   widths are fixed ([`COLUMN_WIDTHS`]) because sizing them to the data would mean holding
//!   every row first; a longer value is written whole and only shifts the rest of its row.

use std::borrow::Cow;
use std::io::{self, Write};

use crime_dataset::{CrimeRecord, HEADER};
use serde::Serialize;

use crate::cli::Format;
use crate::{Dataset, Interpreted};

/// Width of each table column, in characters: the larger of the header and a typical maximum
/// of the field's values in the reference CSV, so that almost every row lines up. Where the
/// longest value of the reference CSV (measured 2026-10-13) is rare, the width stays below it
/// to keep rows readable; those few rows shift.
///
/// | Column | Width | Typical maximum |
/// |---|---|---|
/// | ID | 8 | 8 digits (the longest) |
/// | Case Number | 11 | header (the longest value has 9) |
/// | Date, Updated On | 22 | `MM/DD/YYYY hh:mm:ss AM` |
/// | Block | 38 | the longest |
/// | IUCR, Beat, Ward, Year | 4 | 4 characters |
/// | Primary Type | 33 | `CONCEALED CARRY LICENSE VIOLATION`, the longest |
/// | Description | 45 | most values (the longest has 69) |
/// | Location Description | 40 | most values (the longest has 53) |
/// | Arrest, Domestic, District, FBI Code, Community Area | header | `false`, `003`, `08B`, `77` |
/// | X Coordinate, Y Coordinate | 12 | header (values are 7) |
/// | Latitude, Longitude | 12, 13 | `41.777750747`, `-87.600396201` |
/// | Location | 29 | `(41.777750747, -87.600396201)` |
pub const COLUMN_WIDTHS: [usize; 22] = [
    8, 11, 22, 38, 4, 33, 45, 40, 6, 8, 4, 8, 4, 14, 8, 12, 12, 4, 22, 12, 13, 29,
];

/// Between two table columns.
const COLUMN_GAP: &str = "  ";

/// Writes the records of one result in one format. [`RecordWriter::begin`] writes what comes
/// before the records, [`RecordWriter::write`] one record, [`RecordWriter::finish`] what comes
/// after them; it then flushes and gives the destination back.
pub struct RecordWriter<W: Write> {
    sink: Sink<W>,
    written: u64,
}

enum Sink<W: Write> {
    Table(W),
    Csv(W),
    Json(W),
}

/// The `dataset` member of the JSON output: the version folder read and its manifest's data.
#[derive(Serialize)]
struct DatasetInfo<'a> {
    version: &'a str,
    generated_at: &'a str,
    schema_version: u32,
}

impl<W: Write> RecordWriter<W> {
    /// Starts the output of `query` on `dataset` in `format`: the table header, the CSV header
    /// or the JSON up to the opening of `results`.
    pub fn begin(
        format: Format,
        mut out: W,
        query: &Interpreted,
        dataset: &Dataset,
    ) -> io::Result<Self> {
        let sink = match format {
            Format::Table => {
                write_table_row(&mut out, HEADER.iter().map(|&name| Cow::Borrowed(name)))?;
                let dashes = COLUMN_WIDTHS
                    .iter()
                    .map(|&width| Cow::Owned("-".repeat(width)));
                write_table_row(&mut out, dashes)?;
                Sink::Table(out)
            }
            Format::Csv => {
                write_csv_line(&mut out, HEADER.iter().copied())?;
                Sink::Csv(out)
            }
            Format::Json => {
                let info = DatasetInfo {
                    version: dataset.version(),
                    generated_at: &dataset.manifest().generated_at,
                    schema_version: dataset.manifest().schema_version,
                };
                out.write_all(b"{\"query\":")?;
                serde_json::to_writer(&mut out, query)?;
                out.write_all(b",\"dataset\":")?;
                serde_json::to_writer(&mut out, &info)?;
                out.write_all(b",\"results\":[")?;
                Sink::Json(out)
            }
        };
        Ok(Self { sink, written: 0 })
    }

    pub fn write(&mut self, record: &CrimeRecord) -> io::Result<()> {
        match &mut self.sink {
            Sink::Table(out) => write_table_row(out, text_fields(record))?,
            Sink::Csv(out) => write_csv_line(out, text_fields(record).iter().map(AsRef::as_ref))?,
            Sink::Json(out) => {
                out.write_all(if self.written == 0 { b"\n" } else { b",\n" })?;
                serde_json::to_writer(&mut *out, &JsonRecord::new(record))?;
            }
        }
        self.written += 1;
        Ok(())
    }

    /// Ends the output and flushes it.
    pub fn finish(self) -> io::Result<W> {
        let mut out = match self.sink {
            Sink::Table(out) => out,
            Sink::Csv(out) => out,
            Sink::Json(mut out) => {
                out.write_all(if self.written == 0 {
                    b"]}\n"
                } else {
                    b"\n]}\n"
                })?;
                out
            }
        };
        out.flush()?;
        Ok(out)
    }
}

/// One CSV line written like the source file: each non-empty field between quotes with any
/// quote inside doubled, each empty field bare, `,` between fields and `\n` at the end.
fn write_csv_line<'a>(
    out: &mut impl Write,
    fields: impl IntoIterator<Item = &'a str>,
) -> io::Result<()> {
    for (position, field) in fields.into_iter().enumerate() {
        if position > 0 {
            out.write_all(b",")?;
        }
        if !field.is_empty() {
            out.write_all(b"\"")?;
            if field.contains('"') {
                out.write_all(field.replace('"', "\"\"").as_bytes())?;
            } else {
                out.write_all(field.as_bytes())?;
            }
            out.write_all(b"\"")?;
        }
    }
    out.write_all(b"\n")
}

/// One table row: every value padded to its column, never cut; the last one is not padded.
fn write_table_row<'a>(
    out: &mut impl Write,
    values: impl IntoIterator<Item = Cow<'a, str>>,
) -> io::Result<()> {
    let mut line = String::new();
    for (column, value) in values.into_iter().enumerate() {
        if column > 0 {
            line.push_str(COLUMN_GAP);
        }
        line.push_str(&value);
        if column + 1 < COLUMN_WIDTHS.len() {
            let width = value.chars().count();
            line.extend(std::iter::repeat_n(
                ' ',
                COLUMN_WIDTHS[column].saturating_sub(width),
            ));
        }
    }
    line.push('\n');
    out.write_all(line.as_bytes())
}

/// Every field of `record` as CSV text, in CSV order; "sin valor" is empty.
fn text_fields(record: &CrimeRecord) -> [Cow<'_, str>; 22] {
    fn text(value: &Option<String>) -> Cow<'_, str> {
        Cow::Borrowed(value.as_deref().unwrap_or(""))
    }
    fn number<T: ToString>(value: Option<T>) -> Cow<'static, str> {
        value.map_or(Cow::Borrowed(""), |value| Cow::Owned(value.to_string()))
    }
    fn boolean(value: bool) -> Cow<'static, str> {
        Cow::Borrowed(if value { "true" } else { "false" })
    }
    [
        number(Some(record.id)),
        Cow::Borrowed(&record.case_number),
        Cow::Owned(record.date.to_string()),
        Cow::Borrowed(&record.block),
        Cow::Borrowed(&record.iucr),
        Cow::Borrowed(&record.primary_type),
        Cow::Borrowed(&record.description),
        text(&record.location_description),
        boolean(record.arrest),
        boolean(record.domestic),
        Cow::Borrowed(&record.beat),
        text(&record.district),
        number(record.ward),
        number(record.community_area),
        Cow::Borrowed(&record.fbi_code),
        number(record.x_coordinate),
        number(record.y_coordinate),
        number(Some(record.year)),
        Cow::Owned(record.updated_on.to_string()),
        number(record.latitude),
        number(record.longitude),
        record.location().map_or(Cow::Borrowed(""), Cow::Owned),
    ]
}

/// One JSON record: the CSV header names in CSV order, with typed values.
#[derive(Serialize)]
struct JsonRecord<'a> {
    #[serde(rename = "ID")]
    id: i64,
    #[serde(rename = "Case Number")]
    case_number: &'a str,
    #[serde(rename = "Date")]
    date: String,
    #[serde(rename = "Block")]
    block: &'a str,
    #[serde(rename = "IUCR")]
    iucr: &'a str,
    #[serde(rename = "Primary Type")]
    primary_type: &'a str,
    #[serde(rename = "Description")]
    description: &'a str,
    #[serde(rename = "Location Description")]
    location_description: Option<&'a str>,
    #[serde(rename = "Arrest")]
    arrest: bool,
    #[serde(rename = "Domestic")]
    domestic: bool,
    #[serde(rename = "Beat")]
    beat: &'a str,
    #[serde(rename = "District")]
    district: Option<&'a str>,
    #[serde(rename = "Ward")]
    ward: Option<i64>,
    #[serde(rename = "Community Area")]
    community_area: Option<i64>,
    #[serde(rename = "FBI Code")]
    fbi_code: &'a str,
    #[serde(rename = "X Coordinate")]
    x_coordinate: Option<i64>,
    #[serde(rename = "Y Coordinate")]
    y_coordinate: Option<i64>,
    #[serde(rename = "Year")]
    year: i64,
    #[serde(rename = "Updated On")]
    updated_on: String,
    #[serde(rename = "Latitude")]
    latitude: Option<f64>,
    #[serde(rename = "Longitude")]
    longitude: Option<f64>,
    #[serde(rename = "Location")]
    location: Option<String>,
}

impl<'a> JsonRecord<'a> {
    fn new(record: &'a CrimeRecord) -> Self {
        Self {
            id: record.id,
            case_number: &record.case_number,
            date: record.date.to_string(),
            block: &record.block,
            iucr: &record.iucr,
            primary_type: &record.primary_type,
            description: &record.description,
            location_description: record.location_description.as_deref(),
            arrest: record.arrest,
            domestic: record.domestic,
            beat: &record.beat,
            district: record.district.as_deref(),
            ward: record.ward,
            community_area: record.community_area,
            fbi_code: &record.fbi_code,
            x_coordinate: record.x_coordinate,
            y_coordinate: record.y_coordinate,
            year: record.year,
            updated_on: record.updated_on.to_string(),
            latitude: record.latitude,
            longitude: record.longitude,
            location: record.location(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_column_is_at_least_as_wide_as_its_header() {
        for (name, width) in HEADER.iter().zip(COLUMN_WIDTHS) {
            assert!(name.len() <= width, "{name}: {width}");
        }
    }
}
