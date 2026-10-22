//! The big CSV of scenario R13 (decision KR3 of the `canopy-db-robustez` PRD): the reference CSV
//! with its header once, then two copies of every record, in input order.
//!
//! In each copy a record gets a new numeric ID, above every ID of the original and unique across
//! both copies, and a new case number with the same first two characters (see [`case_numbers`]);
//! every other field is unchanged. The original part is a byte-for-byte copy of the input.
//!
//! The input is read twice and streamed both times: the first pass finds the highest ID and marks
//! the used case numbers, the second writes the result. Memory stays bounded by the case number
//! bitsets (1.25 MB per prefix and width that occurs). The result is written to a temporary file
//! next to the output and renamed only when complete, so a failed run never leaves a partial file
//! with the requested name.

pub mod case_numbers;
pub mod messages;

use std::fs::File;
use std::io::{self, BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use crime_dataset::HEADER;

use crate::case_numbers::CaseNumbers;

const ID: usize = 0;
const CASE_NUMBER: usize = 1;

/// Why the big CSV could not be written; [`Error::message`] gives the text for the operator.
#[derive(Debug)]
pub enum Error {
    OutputExists(PathBuf),
    InputUnreadable(PathBuf),
    HeaderMismatch(PathBuf),
    InvalidRecord { path: PathBuf, line: u64 },
    IdsExhausted,
    CaseNumbersExhausted { prefix: String },
    OutputUnwritable(PathBuf),
}

impl Error {
    pub fn message(&self) -> String {
        match self {
            Self::OutputExists(path) => messages::output_exists(path),
            Self::InputUnreadable(path) => messages::input_unreadable(path),
            Self::HeaderMismatch(path) => messages::header_mismatch(path),
            Self::InvalidRecord { path, line } => messages::invalid_record(path, *line),
            Self::IdsExhausted => messages::ids_exhausted(),
            Self::CaseNumbersExhausted { prefix } => messages::case_numbers_exhausted(prefix),
            Self::OutputUnwritable(path) => messages::output_unwritable(path),
        }
    }
}

/// Writes the big CSV of `input` to `output`, which must not exist; returns its record count.
pub fn build(input: &Path, output: &Path) -> Result<u64, Error> {
    if output.symlink_metadata().is_ok() {
        return Err(Error::OutputExists(output.to_path_buf()));
    }
    let unreadable = |_| Error::InputUnreadable(input.to_path_buf());
    let mut file = File::open(input).map_err(unreadable)?;
    let terminator = line_terminator(&mut file).map_err(unreadable)?;

    // First pass: the highest ID and every used case number.
    let mut case_numbers = CaseNumbers::default();
    let mut max_id: Option<i64> = None;
    let mut rows: u64 = 0;
    for_each_record(&mut file, input, |record| {
        if let Some(id) = std::str::from_utf8(&record[ID])
            .ok()
            .and_then(|id| id.trim().parse::<i64>().ok())
        {
            max_id = Some(max_id.map_or(id, |max| max.max(id)));
        }
        case_numbers.mark(&String::from_utf8_lossy(&record[CASE_NUMBER]));
        rows += 1;
        Ok(())
    })?;

    // Second pass: the original, then both copies, into a temporary file next to the output.
    let unwritable = || Error::OutputUnwritable(output.to_path_buf());
    let dir = match output.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    let temp = tempfile::Builder::new()
        .prefix(".crime-big-csv-")
        .suffix(".tmp")
        .tempfile_in(dir)
        .map_err(|_| unwritable())?;
    let mut sink = BufWriter::with_capacity(1 << 20, temp.as_file());
    copy_original(&mut file, &mut sink, terminator)
        .map_err(|error| io_error(error, input, output))?;

    let mut writer = csv::WriterBuilder::new()
        .quote_style(csv::QuoteStyle::Always)
        .terminator(terminator)
        .from_writer(sink);
    let mut next_id = max_id.unwrap_or(0);
    let mut copy = csv::ByteRecord::new();
    for _ in 0..2 {
        for_each_record(&mut file, input, |record| {
            next_id = next_id.checked_add(1).ok_or(Error::IdsExhausted)?;
            let case_number = case_numbers
                .allocate(&String::from_utf8_lossy(&record[CASE_NUMBER]))
                .map_err(|exhausted| Error::CaseNumbersExhausted {
                    prefix: exhausted.prefix,
                })?;
            copy.clear();
            copy.push_field(next_id.to_string().as_bytes());
            copy.push_field(case_number.as_bytes());
            for field in record.iter().skip(CASE_NUMBER + 1) {
                copy.push_field(field);
            }
            writer
                .write_byte_record(&copy)
                .map_err(|_| Error::OutputUnwritable(output.to_path_buf()))
        })?;
    }
    let sink = writer.into_inner().map_err(|_| unwritable())?;
    drop(sink);
    temp.as_file().sync_all().map_err(|_| unwritable())?;
    temp.persist_noclobber(output).map_err(|_| {
        if output.symlink_metadata().is_ok() {
            Error::OutputExists(output.to_path_buf())
        } else {
            unwritable()
        }
    })?;
    Ok(rows * 3)
}

/// The line terminator of the input's header line: CRLF or LF.
fn line_terminator(file: &mut File) -> io::Result<csv::Terminator> {
    let mut head = Vec::new();
    BufReader::new(&mut *file)
        .take(1 << 16)
        .read_to_end(&mut head)?;
    let crlf = head
        .iter()
        .position(|&byte| byte == b'\n')
        .is_some_and(|index| index > 0 && head[index - 1] == b'\r');
    Ok(if crlf {
        csv::Terminator::CRLF
    } else {
        csv::Terminator::Any(b'\n')
    })
}

/// Calls `visit` with every record of the input after checking its header, from the start of the
/// file.
fn for_each_record(
    file: &mut File,
    path: &Path,
    mut visit: impl FnMut(&csv::ByteRecord) -> Result<(), Error>,
) -> Result<(), Error> {
    file.seek(SeekFrom::Start(0))
        .map_err(|_| Error::InputUnreadable(path.to_path_buf()))?;
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .buffer_capacity(1 << 20)
        .from_reader(&mut *file);
    let header = reader
        .byte_headers()
        .map_err(|_| Error::InputUnreadable(path.to_path_buf()))?;
    if header.len() != HEADER.len()
        || header
            .iter()
            .zip(HEADER)
            .any(|(found, expected)| found != expected.as_bytes())
    {
        return Err(Error::HeaderMismatch(path.to_path_buf()));
    }
    let mut record = csv::ByteRecord::new();
    loop {
        match reader.read_byte_record(&mut record) {
            Ok(true) => visit(&record)?,
            Ok(false) => return Ok(()),
            Err(error) => {
                let line = error.position().map_or(0, csv::Position::line);
                return Err(match error.kind() {
                    csv::ErrorKind::Io(_) => Error::InputUnreadable(path.to_path_buf()),
                    _ => Error::InvalidRecord {
                        path: path.to_path_buf(),
                        line,
                    },
                });
            }
        }
    }
}

/// Copies the input as it is, adding a final line terminator if it has none.
fn copy_original(
    file: &mut File,
    sink: &mut impl Write,
    terminator: csv::Terminator,
) -> io::Result<()> {
    file.seek(SeekFrom::Start(0))?;
    let mut tracker = LastByte {
        inner: sink,
        last: None,
    };
    io::copy(file, &mut tracker)?;
    if tracker.last.is_some_and(|byte| byte != b'\n') {
        tracker.write_all(match terminator {
            csv::Terminator::CRLF => b"\r\n",
            _ => b"\n",
        })?;
    }
    Ok(())
}

/// A writer that remembers the last byte written through it.
struct LastByte<'a, W> {
    inner: &'a mut W,
    last: Option<u8>,
}

impl<W: Write> Write for LastByte<'_, W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let written = self.inner.write(buf)?;
        if written > 0 {
            self.last = Some(buf[written - 1]);
        }
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

/// An I/O error of the copy of the original: a read error is the input's, a write error the
/// output's. `io::copy` does not say which side failed, so the input is probed again.
fn io_error(_error: io::Error, input: &Path, output: &Path) -> Error {
    if File::open(input).is_err() {
        Error::InputUnreadable(input.to_path_buf())
    } else {
        Error::OutputUnwritable(output.to_path_buf())
    }
}
