//! `crime-acceptance select`: the rows of the source CSV that meet a set of conditions, counted,
//! listed by ID or copied, reading the CSV alone. It is the independent side of the query app's
//! acceptance (query app PRD: "un recuento independiente sobre el CSV que no usa código del
//! consultor"), so the condition rules are written here again from the PRD and the feature's
//! decisions, never taken from the query app:
//!
//! - `--where FIELD=VALUE`: FIELD is a CSV header name, matched ignoring ASCII case and the spaces
//!   around it; VALUE is used exactly as given. An empty VALUE is "sin valor": the CSV field is
//!   empty. Values of one field are alternatives (OR); different fields must all hold (AND).
//! - Text and codes compare after lowercasing both sides with `str::to_lowercase` (decision C1);
//!   codes are never padded (`8` is not `008`). ID, Ward, Community Area, X Coordinate,
//!   Y Coordinate and Year compare as integers, Latitude and Longitude as decimals, Arrest and
//!   Domestic as booleans (`true`, `false`, `sí`, `si`, `no`, any case).
//! - Date and Updated On: `AAAA-MM-DD` is that whole civil day and `AAAA-MM-DD hh:mm` that whole
//!   minute. `--range FIELD=FROM..TO` (decision C2) starts at 00:00:00 of FROM without a time, ends
//!   at the last second of TO's day without a time or of its minute with one, and may leave either
//!   end empty; every range must hold, together with the field's `--where` values.
//! - `--startswith FIELD=PREFIX` (startswith PRD): the CSV field begins with PREFIX, both sides
//!   lowercased with `str::to_lowercase` (C1). PREFIX is taken as is, spaces included, and never
//!   padded (`8` does not begin `008`, `00` does); it cannot be empty, and an empty CSV field
//!   begins with no prefix. Only the text columns Case Number, Primary Type, District, IUCR,
//!   Location Description, Beat and FBI Code take one. Prefixes and `--where` values of one field
//!   are alternatives (OR, decision P1); different fields must all hold (AND).
//! - Dates are naive civil seconds of the published text ([`civil`], decision D3): no time zone,
//!   so a time in the spring-forward hour is a valid time.
//!
//! A data row that cannot be read as a record — not 22 fields, not UTF-8, or an ID that is not an
//! integer — is skipped and counted apart; every other row is taken as it is. Rows the generator
//! would reject for another reason, or a repeated ID, are not looked for: the reference CSV has
//! neither (parent PRD, AC-G1).
//!
//! The CSV is read once as a stream. The ID lists keep one ID and one file position per matching
//! row in memory, never the rows: `--rows-out` reads each matching row again from its position.

use std::ffi::OsString;
use std::fmt::Display;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use csv::{ByteRecord, Position, Reader, ReaderBuilder};

use crate::civil;
use crate::fields::HEADER;
use crate::verify::check_header;

/// The command line of `select`, printed after a usage error.
pub const USAGE: &str = "usage: crime-acceptance select --csv <file> [--where FIELD=VALUE]... \
[--startswith FIELD=PREFIX]... [--range Date=FROM..TO]... [--ids-out <file>] [--rows-out <file>]

  --csv <file>            the source CSV
  --where FIELD=VALUE     equality on a CSV column, named in any case; FIELD= means the field is
                          empty; values of one field are OR, different fields are AND
  --startswith FIELD=PREFIX
                          the field begins with PREFIX, in any case, taken as is (no padding);
                          on Case Number, Primary Type, District, IUCR, Location Description,
                          Beat and FBI Code; OR with the field's other values and prefixes
  --range FIELD=FROM..TO  inclusive range on Date or Updated On, each end AAAA-MM-DD [hh:mm] or
                          empty; several ranges are AND
  --ids-out <file>        write the matching IDs, one per line, ascending
  --rows-out <file>       write the header and the matching rows, ascending by ID, each copied
                          byte for byte from the source (quotes and line terminator included)

The number of matching rows goes to the standard output; rows read and skipped to the standard
error. Exit code 0 on success, 1 when a file cannot be read or written, 2 when the command line
is not valid.";

const READ_BUFFER_BYTES: usize = 1 << 20;

/// The columns `--startswith` applies to: the text columns that take conditions.
const PREFIX_COLUMNS: [&str; 7] = [
    "Case Number",
    "Primary Type",
    "District",
    "IUCR",
    "Location Description",
    "Beat",
    "FBI Code",
];

/// How the text of a column is compared.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Text,
    Integer,
    Boolean,
    Decimal,
    DateTime,
}

fn kind(column: usize) -> Kind {
    match HEADER[column] {
        "ID" | "Ward" | "Community Area" | "X Coordinate" | "Y Coordinate" | "Year" => {
            Kind::Integer
        }
        "Arrest" | "Domestic" => Kind::Boolean,
        "Latitude" | "Longitude" => Kind::Decimal,
        "Date" | "Updated On" => Kind::DateTime,
        _ => Kind::Text,
    }
}

/// A searched value, typed by its column.
#[derive(Debug, Clone, PartialEq)]
enum Wanted {
    /// "Sin valor".
    Empty,
    /// Already lowercase.
    Text(String),
    /// A prefix, already lowercase and never empty.
    Prefix(String),
    Integer(i64),
    Boolean(bool),
    Decimal(f64),
    /// Inclusive civil seconds: a day, a minute or a range.
    Between(i64, i64),
}

impl Wanted {
    /// Whether the CSV text of a field is this value.
    fn accepts(&self, text: &str) -> bool {
        match self {
            Wanted::Empty => text.is_empty(),
            Wanted::Text(wanted) => text.to_lowercase() == *wanted,
            Wanted::Prefix(prefix) => !text.is_empty() && text.to_lowercase().starts_with(prefix),
            Wanted::Integer(wanted) => text.parse::<i64>().ok() == Some(*wanted),
            Wanted::Boolean(wanted) => {
                let stored = match text.to_lowercase().as_str() {
                    "true" => Some(true),
                    "false" => Some(false),
                    _ => None,
                };
                stored == Some(*wanted)
            }
            Wanted::Decimal(wanted) => text.parse::<f64>().ok() == Some(*wanted),
            Wanted::Between(first, last) => {
                civil::parse_civil(text).is_some_and(|seconds| (*first..=*last).contains(&seconds))
            }
        }
    }
}

/// The conditions of one run. Without any, every row matches.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Selection {
    /// Each conditioned column with its alternatives (values and prefixes), in first-appearance
    /// order.
    equalities: Vec<(usize, Vec<Wanted>)>,
    /// Each range with its column; all must hold.
    ranges: Vec<(usize, Wanted)>,
}

impl Selection {
    /// Adds `FIELD=VALUE`.
    pub fn add_where(&mut self, condition: &str) -> Result<(), String> {
        let (name, value) = condition
            .split_once('=')
            .ok_or_else(|| format!("--where {condition:?} is not FIELD=VALUE"))?;
        let column = column(name)?;
        let wanted = wanted(column, value)?;
        self.add_alternative(column, wanted);
        Ok(())
    }

    /// Adds `FIELD=PREFIX`.
    pub fn add_startswith(&mut self, condition: &str) -> Result<(), String> {
        let (name, prefix) = condition
            .split_once('=')
            .ok_or_else(|| format!("--startswith {condition:?} is not FIELD=PREFIX"))?;
        let column = column(name)?;
        if !PREFIX_COLUMNS.contains(&HEADER[column]) {
            return Err(format!(
                "--startswith {condition:?}: prefixes apply to the text columns {} only",
                PREFIX_COLUMNS.join(", ")
            ));
        }
        if prefix.is_empty() {
            return Err(format!("--startswith {condition:?}: the prefix is empty"));
        }
        self.add_alternative(column, Wanted::Prefix(prefix.to_lowercase()));
        Ok(())
    }

    /// One more alternative of `column`: its values and prefixes are OR.
    fn add_alternative(&mut self, column: usize, wanted: Wanted) {
        match self.equalities.iter_mut().find(|(c, _)| *c == column) {
            Some((_, alternatives)) => alternatives.push(wanted),
            None => self.equalities.push((column, vec![wanted])),
        }
    }

    /// Adds `FIELD=FROM..TO`.
    pub fn add_range(&mut self, condition: &str) -> Result<(), String> {
        let malformed = || format!("--range {condition:?} is not FIELD=FROM..TO");
        let (name, bounds) = condition.split_once('=').ok_or_else(malformed)?;
        let column = column(name)?;
        if kind(column) != Kind::DateTime {
            return Err(format!(
                "--range {condition:?}: ranges apply to Date and Updated On only"
            ));
        }
        let (from, to) = bounds.split_once("..").ok_or_else(malformed)?;
        if from.is_empty() && to.is_empty() {
            return Err(format!("--range {condition:?} needs at least one end"));
        }
        let first = if from.is_empty() {
            i64::MIN
        } else {
            window(from)?.0
        };
        let last = if to.is_empty() {
            i64::MAX
        } else {
            window(to)?.1
        };
        if first > last {
            return Err(format!("--range {condition:?} starts after it ends"));
        }
        self.ranges.push((column, Wanted::Between(first, last)));
        Ok(())
    }

    /// Whether a row, its fields in [`HEADER`] order, meets every condition.
    pub fn matches(&self, fields: &[&str]) -> bool {
        self.equalities.iter().all(|(column, alternatives)| {
            alternatives
                .iter()
                .any(|wanted| wanted.accepts(fields[*column]))
        }) && self
            .ranges
            .iter()
            .all(|(column, range)| range.accepts(fields[*column]))
    }
}

/// The column named `name`, ignoring ASCII case and the spaces around it.
fn column(name: &str) -> Result<usize, String> {
    let name = name.trim();
    HEADER
        .iter()
        .position(|column| column.eq_ignore_ascii_case(name))
        .ok_or_else(|| {
            format!(
                "unknown field {name:?}; the fields are the CSV columns: {}",
                HEADER.join(", ")
            )
        })
}

/// `value` typed for `column`; empty is "sin valor".
fn wanted(column: usize, value: &str) -> Result<Wanted, String> {
    if value.is_empty() {
        return Ok(Wanted::Empty);
    }
    let invalid = |expected: &str| {
        format!(
            "{value:?} is not valid for {:?}: expected {expected}",
            HEADER[column]
        )
    };
    Ok(match kind(column) {
        Kind::Text => Wanted::Text(value.to_lowercase()),
        Kind::Integer => Wanted::Integer(value.parse().map_err(|_| invalid("an integer"))?),
        Kind::Boolean => match value.to_lowercase().as_str() {
            "true" | "sí" | "si" => Wanted::Boolean(true),
            "false" | "no" => Wanted::Boolean(false),
            _ => return Err(invalid("true, false, sí or no")),
        },
        Kind::Decimal => match value.parse::<f64>() {
            Ok(decimal) if decimal.is_finite() => Wanted::Decimal(decimal),
            _ => return Err(invalid("a decimal number")),
        },
        Kind::DateTime => {
            let (first, last) = window(value)?;
            Wanted::Between(first, last)
        }
    })
}

/// The first and last civil second of the day `AAAA-MM-DD` or of the minute `AAAA-MM-DD hh:mm`.
fn window(text: &str) -> Result<(i64, i64), String> {
    parse_window(text)
        .ok_or_else(|| format!("{text:?} is not a date: expected AAAA-MM-DD or AAAA-MM-DD hh:mm"))
}

fn parse_window(text: &str) -> Option<(i64, i64)> {
    let number = |part: &str, digits: usize| -> Option<i64> {
        (part.len() == digits && part.bytes().all(|byte| byte.is_ascii_digit()))
            .then(|| part.parse().ok())
            .flatten()
    };
    let (date, time) = match text.split_once(' ') {
        Some((date, time)) => (date, Some(time)),
        None => (text, None),
    };
    let mut parts = date.split('-');
    let year = number(parts.next()?, 4)?;
    let month = number(parts.next()?, 2)?;
    let day = number(parts.next()?, 2)?;
    if parts.next().is_some() {
        return None;
    }
    let (hour, minute, rest) = match time {
        None => (0, 0, 86_399),
        Some(time) => {
            let (hour, minute) = time.split_once(':')?;
            (number(hour, 2)?, number(minute, 2)?, 59)
        }
    };
    let first = civil::civil_seconds(year, month, day, hour, minute, 0)?;
    Some((first, first + rest))
}

/// What the command line of `select` asks for.
#[derive(Debug)]
pub struct Request {
    pub csv: PathBuf,
    pub selection: Selection,
    pub ids_out: Option<PathBuf>,
    pub rows_out: Option<PathBuf>,
}

/// The arguments after `select`.
pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Request, String> {
    let mut csv = None;
    let mut selection = Selection::default();
    let mut ids_out = None;
    let mut rows_out = None;
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        let name = arg.to_string_lossy();
        let mut value = |what: &str| args.next().ok_or_else(|| format!("{name} needs {what}"));
        let text = |value: OsString| {
            value
                .into_string()
                .map_err(|value| format!("{} is not valid UTF-8", value.to_string_lossy()))
        };
        match arg.to_str() {
            Some("--csv") => csv = Some(PathBuf::from(value("a file")?)),
            Some("--where") => selection.add_where(&text(value("FIELD=VALUE")?)?)?,
            Some("--startswith") => selection.add_startswith(&text(value("FIELD=PREFIX")?)?)?,
            Some("--range") => selection.add_range(&text(value("FIELD=FROM..TO")?)?)?,
            Some("--ids-out") => ids_out = Some(PathBuf::from(value("a file")?)),
            Some("--rows-out") => rows_out = Some(PathBuf::from(value("a file")?)),
            _ => return Err(format!("unknown argument {name}")),
        }
    }
    Ok(Request {
        csv: csv.ok_or("--csv is required")?,
        selection,
        ids_out,
        rows_out,
    })
}

/// The figures of one run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Summary {
    /// Data rows in the CSV, skipped ones included.
    pub rows: u64,
    pub matching: u64,
    /// Rows that cannot be read as a record.
    pub skipped: u64,
}

/// Reads the CSV, counts the matching rows and writes the requested lists. The error is the
/// file that could not be read or written.
pub fn run(request: &Request) -> Result<Summary, String> {
    let path = &request.csv;
    let unreadable = |error: &dyn Display| format!("{} cannot be read: {error}", path.display());
    let file = File::open(path).map_err(|error| unreadable(&error))?;
    let mut reader = ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .buffer_capacity(READ_BUFFER_BYTES)
        .from_reader(BufReader::new(file));
    let mut record = ByteRecord::new();
    if !reader
        .read_byte_record(&mut record)
        .map_err(|error| unreadable(&error))?
    {
        return Err(format!("{} is empty", path.display()));
    }
    check_header(&record).map_err(|error| format!("{}: {error}", path.display()))?;

    let listed = request.ids_out.is_some() || request.rows_out.is_some();
    let mut found: Vec<(i64, Position)> = Vec::new();
    let mut summary = Summary::default();
    loop {
        let position = reader.position().clone();
        if !reader
            .read_byte_record(&mut record)
            .map_err(|error| unreadable(&error))?
        {
            break;
        }
        summary.rows += 1;
        let Some((id, fields)) = data_row(&record) else {
            summary.skipped += 1;
            continue;
        };
        if request.selection.matches(&fields) {
            summary.matching += 1;
            if listed {
                found.push((id, position));
            }
        }
    }
    // Stable: rows sharing an ID keep their file order.
    found.sort_by_key(|(id, _)| *id);

    if let Some(out) = &request.ids_out {
        write_out(out, |writer| {
            found
                .iter()
                .try_for_each(|(id, _)| writeln!(writer, "{id}").map_err(|e| e.to_string()))
        })?;
    }
    if let Some(out) = &request.rows_out {
        write_out(out, |writer| {
            write_rows(writer, path, &found, &mut reader, &unreadable)
        })?;
    }
    Ok(summary)
}

/// The ID and the fields of a data row, or `None` when it cannot be read as a record: not 22
/// fields, not UTF-8, or an ID that is not an integer.
fn data_row(record: &ByteRecord) -> Option<(i64, Vec<&str>)> {
    if record.len() != HEADER.len() {
        return None;
    }
    let fields: Vec<&str> = record
        .iter()
        .map(|field| std::str::from_utf8(field).ok())
        .collect::<Option<_>>()?;
    let id = fields[0].parse().ok()?;
    Some((id, fields))
}

/// The header line of the source, with its line terminator (CRLF or LF), as raw bytes.
fn source_header_line(path: &Path) -> std::io::Result<Vec<u8>> {
    let mut line = Vec::new();
    BufReader::new(File::open(path)?).read_until(b'\n', &mut line)?;
    Ok(line)
}

/// The header line and every row at `found`, copied byte for byte from the source, each row
/// ended by the header's line terminator (the source uses one terminator throughout).
fn write_rows(
    writer: &mut BufWriter<File>,
    path: &Path,
    found: &[(i64, Position)],
    reader: &mut Reader<BufReader<File>>,
    unreadable: &dyn Fn(&dyn Display) -> String,
) -> Result<(), String> {
    let header = source_header_line(path).map_err(|error| unreadable(&error))?;
    let terminator: &[u8] = if header.ends_with(b"\r\n") {
        b"\r\n"
    } else {
        b"\n"
    };
    writer.write_all(&header).map_err(|e| e.to_string())?;
    let mut source = File::open(path).map_err(|error| unreadable(&error))?;
    let mut record = ByteRecord::new();
    let mut bytes = Vec::new();
    for (id, position) in found {
        reader
            .seek(position.clone())
            .map_err(|error| unreadable(&error))?;
        if !reader
            .read_byte_record(&mut record)
            .map_err(|error| unreadable(&error))?
        {
            return Err(unreadable(&format!("the row of ID {id} is gone")));
        }
        let (start, end) = (position.byte(), reader.position().byte());
        let length = usize::try_from(end - start).map_err(|error| unreadable(&error))?;
        bytes.resize(length, 0);
        source
            .seek(SeekFrom::Start(start))
            .and_then(|_| source.read_exact(&mut bytes))
            .map_err(|error| unreadable(&error))?;
        // The span can hold the end of the previous terminator (the reader stops a record
        // after `\r` of a CRLF) and not all of its own: the row itself never starts or ends
        // with CR or LF, because a quoted field ends with its quote.
        let is_break = |byte: &u8| *byte == b'\r' || *byte == b'\n';
        let first = bytes.iter().position(|byte| !is_break(byte)).unwrap_or(0);
        let last = bytes
            .iter()
            .rposition(|byte| !is_break(byte))
            .map_or(0, |at| at + 1);
        writer
            .write_all(&bytes[first..last.max(first)])
            .and_then(|()| writer.write_all(terminator))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Creates `out`, writes it with `write` and flushes it; any failure names the file.
fn write_out(
    out: &Path,
    write: impl FnOnce(&mut BufWriter<File>) -> Result<(), String>,
) -> Result<(), String> {
    let unwritable = |error: &dyn Display| format!("{} cannot be written: {error}", out.display());
    let mut writer = BufWriter::new(File::create(out).map_err(|error| unwritable(&error))?);
    write(&mut writer).map_err(|error| unwritable(&error))?;
    writer.flush().map_err(|error| unwritable(&error))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fields::HEADER;

    /// The first data row of the reference CSV.
    const SAMPLE_ROW: [&str; 22] = [
        "14348063",
        "JK438266",
        "10/03/2026 12:00:00 AM",
        "064XX S ELLIS AVE",
        "1090",
        "ARSON",
        "ATTEMPT ARSON",
        "VEHICLE NON-COMMERCIAL",
        "false",
        "false",
        "0314",
        "003",
        "20",
        "42",
        "09",
        "1184164",
        "1862455",
        "2026",
        "10/10/2026 03:42:19 PM",
        "41.777750747",
        "-87.600396201",
        "(41.777750747, -87.600396201)",
    ];

    /// The sample row with one column replaced, by name.
    fn row_with(name: &str, value: &'static str) -> [&'static str; 22] {
        let mut row = SAMPLE_ROW;
        let column = HEADER
            .iter()
            .position(|&column| column == name)
            .unwrap_or_else(|| panic!("{name} is a column"));
        row[column] = value;
        row
    }

    fn selection(wheres: &[&str], ranges: &[&str]) -> Selection {
        let mut selection = Selection::default();
        for condition in wheres {
            selection
                .add_where(condition)
                .unwrap_or_else(|error| panic!("{condition}: {error}"));
        }
        for condition in ranges {
            selection
                .add_range(condition)
                .unwrap_or_else(|error| panic!("{condition}: {error}"));
        }
        selection
    }

    /// Whether `--where condition` accepts the sample row with `name` set to `value`.
    fn accepts(condition: &str, name: &str, value: &'static str) -> bool {
        selection(&[condition], &[]).matches(&row_with(name, value))
    }

    /// Whether `--range condition` accepts the sample row with Date set to `date`.
    fn in_range(condition: &str, date: &'static str) -> bool {
        selection(&[], &[condition]).matches(&row_with("Date", date))
    }

    #[test]
    fn text_compares_after_lowercasing_both_sides() {
        for wanted in [
            "Primary Type=theft",
            "Primary Type=Theft",
            "Primary Type=THEFT",
        ] {
            for stored in ["THEFT", "Theft", "theft"] {
                assert!(accepts(wanted, "Primary Type", stored), "{wanted} {stored}");
            }
            assert!(
                !accepts(wanted, "Primary Type", "MOTOR VEHICLE THEFT"),
                "{wanted}"
            );
        }
        assert!(accepts("Case Number=ja123", "Case Number", "JA123"));
        assert!(accepts(
            "Location Description=street",
            "Location Description",
            "STREET"
        ));
        assert!(accepts(
            "Primary Type=i\u{307}stanbul",
            "Primary Type",
            "İSTANBUL"
        ));
        assert!(
            !accepts("Primary Type= THEFT", "Primary Type", "THEFT"),
            "values are not trimmed"
        );
    }

    #[test]
    fn field_names_ignore_ascii_case_and_surrounding_spaces() {
        for condition in [
            "primary type=THEFT",
            " PRIMARY TYPE =THEFT",
            "Primary Type=THEFT",
        ] {
            assert!(accepts(condition, "Primary Type", "THEFT"), "{condition}");
        }
    }

    #[test]
    fn codes_are_never_padded() {
        assert!(!accepts("District=8", "District", "008"));
        assert!(!accepts("District=008", "District", "8"));
        assert!(accepts("District=008", "District", "008"));
        assert!(!accepts("Beat=314", "Beat", "0314"));
        assert!(accepts("FBI Code=08b", "FBI Code", "08B"));
    }

    #[test]
    fn an_empty_value_is_sin_valor() {
        assert!(accepts("Community Area=", "Community Area", ""));
        assert!(!accepts("Community Area=", "Community Area", "42"));
        assert!(!accepts("Community Area=0", "Community Area", ""));
        assert!(accepts("District=", "District", ""));
        assert!(!accepts("District=", "District", "003"));
        assert!(accepts("Latitude=", "Latitude", ""));
        assert!(
            !accepts("Primary Type=", "Primary Type", "THEFT"),
            "a mandatory field"
        );
    }

    #[test]
    fn integer_fields_compare_as_integers() {
        assert!(accepts("Ward=20", "Ward", "20"));
        assert!(accepts("Ward=+20", "Ward", "20"));
        assert!(accepts("Ward=020", "Ward", "20"));
        assert!(!accepts("Ward=2", "Ward", "20"));
        assert!(accepts("ID=14348063", "ID", "14348063"));
        assert!(accepts("Year=2026", "Year", "2026"));
        assert!(accepts("X Coordinate=1184164", "X Coordinate", "1184164"));
        assert!(accepts("Y Coordinate=-5", "Y Coordinate", "-5"));
        assert!(accepts("Community Area=8", "Community Area", "8"));
    }

    #[test]
    fn booleans_accept_true_false_si_and_no_in_any_case() {
        for wanted in ["true", "TRUE", "sí", "SÍ", "si", "Si"] {
            assert!(
                accepts(&format!("Arrest={wanted}"), "Arrest", "true"),
                "{wanted}"
            );
            assert!(
                !accepts(&format!("Arrest={wanted}"), "Arrest", "false"),
                "{wanted}"
            );
        }
        for wanted in ["false", "False", "no", "NO"] {
            assert!(
                accepts(&format!("Domestic={wanted}"), "Domestic", "false"),
                "{wanted}"
            );
            assert!(
                !accepts(&format!("Domestic={wanted}"), "Domestic", "true"),
                "{wanted}"
            );
        }
    }

    #[test]
    fn decimals_compare_as_numbers() {
        assert!(accepts("Latitude=41.777750747", "Latitude", "41.777750747"));
        assert!(accepts("Latitude=41.80", "Latitude", "41.8"));
        assert!(accepts("Latitude=41.8", "Latitude", "41.80"));
        assert!(!accepts("Latitude=41.8", "Latitude", "41.800001"));
        assert!(accepts(
            "Longitude=-87.600396201",
            "Longitude",
            "-87.600396201"
        ));
    }

    #[test]
    fn a_date_without_time_is_the_whole_civil_day() {
        let condition = "Date=2026-10-03";
        assert!(accepts(condition, "Date", "10/03/2026 12:00:00 AM"));
        assert!(accepts(condition, "Date", "10/03/2026 11:59:59 PM"));
        assert!(!accepts(condition, "Date", "10/02/2026 11:59:59 PM"));
        assert!(!accepts(condition, "Date", "10/04/2026 12:00:00 AM"));
        assert!(accepts(
            "Updated On=2026-10-10",
            "Updated On",
            "10/10/2026 03:42:19 PM"
        ));
        assert!(!accepts(
            "Updated On=2026-10-11",
            "Updated On",
            "10/10/2026 03:42:19 PM"
        ));
    }

    #[test]
    fn a_date_with_time_is_the_whole_minute() {
        let condition = "Date=2026-10-03 15:42";
        assert!(accepts(condition, "Date", "10/03/2026 03:42:00 PM"));
        assert!(accepts(condition, "Date", "10/03/2026 03:42:59 PM"));
        assert!(!accepts(condition, "Date", "10/03/2026 03:41:59 PM"));
        assert!(!accepts(condition, "Date", "10/03/2026 03:43:00 PM"));
        assert!(!accepts(condition, "Date", "10/03/2026 03:42:00 AM"));
        assert!(accepts(
            "Date=2026-10-03 00:00",
            "Date",
            "10/03/2026 12:00:30 AM"
        ));
        assert!(accepts(
            "Date=2026-10-03 12:05",
            "Date",
            "10/03/2026 12:05:30 PM"
        ));
    }

    #[test]
    fn a_time_in_the_spring_forward_hour_is_a_valid_civil_time() {
        assert!(accepts(
            "Date=2015-03-08 02:30",
            "Date",
            "03/08/2015 02:30:00 AM"
        ));
        assert!(in_range(
            "Date=2015-03-08 02:00..2015-03-08 02:59",
            "03/08/2015 02:59:59 AM"
        ));
    }

    #[test]
    fn a_range_of_days_includes_its_first_and_last_second_only() {
        let month = "Date=2026-10-01..2026-10-31";
        assert!(in_range(month, "10/01/2026 12:00:00 AM"));
        assert!(in_range(month, "10/31/2026 11:59:59 PM"));
        assert!(in_range(month, "10/15/2026 01:00:00 PM"));
        assert!(!in_range(month, "09/30/2026 11:59:59 PM"));
        assert!(!in_range(month, "11/01/2026 12:00:00 AM"));
    }

    #[test]
    fn a_range_with_times_starts_at_its_first_minute_and_includes_its_last_minute() {
        let minutes = "Date=2026-10-01 10:15..2026-10-01 10:20";
        assert!(in_range(minutes, "10/01/2026 10:15:00 AM"));
        assert!(in_range(minutes, "10/01/2026 10:20:59 AM"));
        assert!(!in_range(minutes, "10/01/2026 10:14:59 AM"));
        assert!(!in_range(minutes, "10/01/2026 10:21:00 AM"));
    }

    #[test]
    fn either_end_of_a_range_may_be_open() {
        assert!(in_range("Date=2026-10-01..", "10/01/2026 12:00:00 AM"));
        assert!(in_range("Date=2026-10-01..", "12/31/9999 11:59:59 PM"));
        assert!(!in_range("Date=2026-10-01..", "09/30/2026 11:59:59 PM"));
        assert!(in_range("Date=..2026-10-01", "01/01/0001 12:00:00 AM"));
        assert!(in_range("Date=..2026-10-01", "10/01/2026 11:59:59 PM"));
        assert!(!in_range("Date=..2026-10-01", "10/02/2026 12:00:00 AM"));
        assert!(
            !in_range("Date=2026-10-01..", ""),
            "an empty date is in no range"
        );
    }

    #[test]
    fn repeated_fields_are_or_and_different_fields_are_and() {
        let either = selection(&["Primary Type=THEFT", "primary type=ARSON"], &[]);
        assert!(either.matches(&row_with("Primary Type", "ARSON")));
        assert!(either.matches(&row_with("Primary Type", "theft")));
        assert!(!either.matches(&row_with("Primary Type", "BATTERY")));

        let both = selection(&["Primary Type=ARSON", "District=003"], &[]);
        assert!(both.matches(&SAMPLE_ROW));
        assert!(!both.matches(&row_with("District", "008")));
        assert!(!both.matches(&row_with("Primary Type", "THEFT")));

        let days = selection(&["Date=2026-10-03", "Date=2026-10-05"], &[]);
        assert!(days.matches(&row_with("Date", "10/05/2026 08:00:00 AM")));
        assert!(!days.matches(&row_with("Date", "10/04/2026 08:00:00 AM")));
    }

    #[test]
    fn every_range_must_hold_together_with_the_date_values() {
        let ranges = selection(&[], &["Date=2026-10-01..2026-10-10", "Date=2026-10-05.."]);
        assert!(ranges.matches(&row_with("Date", "10/05/2026 12:00:00 AM")));
        assert!(ranges.matches(&row_with("Date", "10/10/2026 11:59:59 PM")));
        assert!(!ranges.matches(&row_with("Date", "10/04/2026 11:59:59 PM")));
        assert!(!ranges.matches(&row_with("Date", "10/11/2026 12:00:00 AM")));

        let day_in_range = selection(&["Date=2026-10-03"], &["Date=2026-10-01..2026-10-02"]);
        assert!(!day_in_range.matches(&SAMPLE_ROW));
    }

    #[test]
    fn no_condition_accepts_every_row() {
        assert!(Selection::default().matches(&SAMPLE_ROW));
    }

    #[test]
    fn conditions_that_cannot_be_applied_are_usage_errors() {
        let wrong_wheres = [
            "Primary Type",
            "Color=red",
            "=THEFT",
            "Ward=abc",
            "Ward=1.5",
            "Latitude=north",
            "Latitude=NaN",
            "Arrest=yes",
            "Date=2026-10-3",
            "Date=10/03/2026",
            "Date=2026-02-30",
            "Date=2026-10-03 24:00",
            "Date=2026-10-03 10:60",
            "Date=2026-10-03 10:15:00",
            "Date=2026-10-03T10:15",
        ];
        for condition in wrong_wheres {
            assert!(
                Selection::default().add_where(condition).is_err(),
                "--where {condition}"
            );
        }
        let wrong_ranges = [
            "Date",
            "Date=2026-10-01",
            "Date=..",
            "Ward=1..2",
            "Color=2026-10-01..",
            "Date=2026-10-02..2026-10-01",
            "Date=2026-10-01 10:16..2026-10-01 10:15",
            "Date=2026-13-01..",
        ];
        for condition in wrong_ranges {
            assert!(
                Selection::default().add_range(condition).is_err(),
                "--range {condition}"
            );
        }
        let message = Selection::default()
            .add_where("Color=red")
            .expect_err("an unknown field");
        assert!(
            message.contains("Color") && message.contains("Primary Type"),
            "{message}"
        );
    }

    #[test]
    fn the_command_line_needs_a_csv_and_well_formed_options() {
        let args = |list: &[&str]| list.iter().map(OsString::from).collect::<Vec<_>>();

        let request = parse(args(&[
            "--csv",
            "crimes.csv",
            "--where",
            "Primary Type=THEFT",
            "--range",
            "Date=2026-10-01..",
            "--ids-out",
            "ids.txt",
            "--rows-out",
            "rows.csv",
        ]))
        .expect("a valid command line");
        assert_eq!(request.csv, PathBuf::from("crimes.csv"));
        assert_eq!(request.ids_out, Some(PathBuf::from("ids.txt")));
        assert_eq!(request.rows_out, Some(PathBuf::from("rows.csv")));
        assert_eq!(
            request.selection,
            selection(&["Primary Type=THEFT"], &["Date=2026-10-01.."])
        );

        for wrong in [
            &[][..],
            &["--where", "Primary Type=THEFT"],
            &["--csv"],
            &["--csv", "crimes.csv", "--where"],
            &["--csv", "crimes.csv", "--where", "Ward=abc"],
            &["--csv", "crimes.csv", "--range", "Ward=1..2"],
            &["--csv", "crimes.csv", "--limit", "5"],
        ] {
            assert!(parse(args(wrong)).is_err(), "{wrong:?}");
        }
    }
}
