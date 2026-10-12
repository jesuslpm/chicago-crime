//! Streaming ingestion of the source CSV: the header is checked first, then every row becomes a
//! [`ValidRow`] or a [`Rejection`], while the source fingerprint is computed in the same read.

use std::borrow::Cow;
use std::collections::VecDeque;
use std::io::{self, Chain, Cursor, Read};
use std::path::{Path, PathBuf};

use crime_dataset::{CrimeRecord, HEADER, RecordCounts, SourceInfo};
use csv::ByteRecord;

use crate::Abort;
use crate::hashing::HashingReader;
use crate::log::RunLog;
use crate::validation::{Rejection, validate_row};

const UTF8_BOM: &[u8] = b"\xEF\xBB\xBF";

/// Large reads keep the 2.3 GB scan cheap; the csv default is 8 KiB.
const READ_BUFFER_BYTES: usize = 1 << 20;

/// A record accepted from the CSV, with the line where its row starts (the header is line 1).
///
/// The line breaks ties between repeated IDs so that the first occurrence in the file wins.
#[derive(Debug, Clone, PartialEq)]
pub struct ValidRow {
    pub line: u64,
    pub record: CrimeRecord,
}

/// What one row of the CSV became.
pub type RowOutcome = Result<ValidRow, Rejection>;

/// What the CSV parser reads, from the bottom up: every byte of the file goes through the hashing
/// reader; a leading BOM is then dropped; line feeds are tracked in what is left, which is
/// exactly the byte stream the parser's positions refer to.
type Source<R> = LineFeeds<Chain<Cursor<Vec<u8>>, HashingReader<R>>>;

/// Reads the CSV one row at a time.
pub struct CsvIngest<R: Read> {
    input: PathBuf,
    reader: csv::Reader<Source<R>>,
    row: ByteRecord,
    counts: RecordCounts,
}

impl<R: Read> CsvIngest<R> {
    /// Starts reading `source`, the content of the file `input`, and checks the header before
    /// any row is read.
    pub fn new(input: &Path, source: R) -> Result<Self, Abort> {
        let mut hashed = HashingReader::new(source);
        let start = skip_bom(&mut hashed).map_err(|_| Abort::InputUnreadable {
            path: input.to_path_buf(),
        })?;
        let reader = csv::ReaderBuilder::new()
            // The header is read as a plain record so it can be compared column by column.
            .has_headers(false)
            // Rows with another column count are rejected by validation, not by the parser.
            .flexible(true)
            .buffer_capacity(READ_BUFFER_BYTES)
            .from_reader(LineFeeds::new(Cursor::new(start).chain(hashed)));
        let mut ingest = Self {
            input: input.to_path_buf(),
            reader,
            row: ByteRecord::new(),
            counts: RecordCounts {
                valid: 0,
                rejected: 0,
            },
        };
        // At the end of the file the record is left empty, which reports column 1 as missing.
        ingest.read_row()?;
        check_header(&ingest.row)?;
        Ok(ingest)
    }

    /// The next row as a valid record or a rejection; `None` at the end of the file.
    ///
    /// Only an I/O error aborts: a bad row is an outcome, not an error (decision D5).
    pub fn next_row(&mut self) -> Result<Option<RowOutcome>, Abort> {
        if !self.read_row()? {
            return Ok(None);
        }
        let line = self.start_line();
        let outcome = validate_row(line, &self.row).map(|record| ValidRow { line, record });
        match outcome {
            Ok(_) => self.counts.valid += 1,
            Err(_) => self.counts.rejected += 1,
        }
        Ok(Some(outcome))
    }

    /// The next valid row, logging every rejection found on the way; `None` at the end of the file.
    pub fn next_valid(&mut self, log: &mut RunLog) -> Result<Option<ValidRow>, Abort> {
        while let Some(outcome) = self.next_row()? {
            match outcome {
                Ok(row) => return Ok(Some(row)),
                Err(rejection) => log.rejection(&rejection)?,
            }
        }
        Ok(None)
    }

    /// Valid and rejected rows so far.
    pub fn counts(&self) -> RecordCounts {
        self.counts
    }

    /// Identity of the source: name, size and SHA-256 of the whole file.
    pub fn finish(self) -> Result<SourceInfo, Abort> {
        let (_, hashed) = self.reader.into_inner().inner.into_inner();
        let fingerprint = hashed.finish().map_err(|_| Abort::InputUnreadable {
            path: self.input.clone(),
        })?;
        let file_name = self.input.file_name().map_or_else(
            || self.input.to_string_lossy(),
            |name| name.to_string_lossy(),
        );
        Ok(SourceInfo {
            file_name: file_name.into_owned(),
            size_bytes: fingerprint.size_bytes,
            sha256: fingerprint.sha256,
        })
    }

    /// Reads the next record into `self.row`; `false` at the end of the file.
    fn read_row(&mut self) -> Result<bool, Abort> {
        self.reader
            .read_byte_record(&mut self.row)
            .map_err(|_| Abort::InputUnreadable {
                path: self.input.clone(),
            })
    }

    /// Line where the record just read starts, derived from where it ends.
    ///
    /// The parser's own start position is taken before it skips the line feed of a CRLF ending
    /// or blank lines, so it can fall behind. Its position after the record counts every line
    /// feed consumed so far; removing the ones inside quoted fields and the terminator, if it was
    /// a line feed, leaves the line of the first byte of the record.
    fn start_line(&mut self) -> u64 {
        let end = self.reader.position().clone();
        let inner_line_feeds = self
            .row
            .as_slice()
            .iter()
            .filter(|&&byte| byte == b'\n')
            .count();
        let ends_with_line_feed =
            end.byte() > 0 && self.reader.get_mut().is_line_feed(end.byte() - 1);
        end.line() - inner_line_feeds as u64 - u64::from(ends_with_line_feed)
    }
}

/// Remembers the offsets of the line feeds that the parser has buffered but not consumed yet.
struct LineFeeds<R> {
    inner: R,
    offset: u64,
    pending: VecDeque<u64>,
}

impl<R> LineFeeds<R> {
    fn new(inner: R) -> Self {
        Self {
            inner,
            offset: 0,
            pending: VecDeque::new(),
        }
    }

    /// Whether the byte at `offset` is a line feed. Offsets must be asked in increasing order,
    /// because everything before `offset` is forgotten.
    fn is_line_feed(&mut self, offset: u64) -> bool {
        while self
            .pending
            .front()
            .is_some_and(|&pending| pending < offset)
        {
            self.pending.pop_front();
        }
        self.pending.front() == Some(&offset)
    }
}

impl<R: Read> Read for LineFeeds<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let read = self.inner.read(buf)?;
        let start = self.offset;
        self.pending.extend(
            buf[..read]
                .iter()
                .enumerate()
                .filter(|&(_, &byte)| byte == b'\n')
                .map(|(index, _)| start + index as u64),
        );
        self.offset += read as u64;
        Ok(read)
    }
}

/// Reads the first bytes of the source and drops them if they are a UTF-8 BOM.
fn skip_bom(source: &mut impl Read) -> io::Result<Vec<u8>> {
    let mut start = Vec::with_capacity(UTF8_BOM.len());
    source
        .by_ref()
        .take(UTF8_BOM.len() as u64)
        .read_to_end(&mut start)?;
    if start == UTF8_BOM {
        start.clear();
    }
    Ok(start)
}

/// The header must equal [`HEADER`] exactly; the first differing column is reported.
fn check_header(header: &ByteRecord) -> Result<(), Abort> {
    for index in 0..HEADER.len().max(header.len()) {
        let expected = HEADER.get(index).copied();
        let found = header.get(index).map(String::from_utf8_lossy);
        if expected != found.as_deref() {
            return Err(Abort::HeaderMismatch {
                column: index + 1,
                expected: expected.unwrap_or_default().to_owned(),
                found: found.map(Cow::into_owned).unwrap_or_default(),
            });
        }
    }
    Ok(())
}
