//! Generator of the Chicago crime dataset.
//!
//! The binary only parses the command line; every stage lives here so it can be tested:
//!
//! - [`cli`]: options and the resolved [`Config`];
//! - [`preflight`]: checks that run before any row is read;
//! - [`ingest`] and [`validation`]: streaming the CSV into typed records or rejections;
//! - [`hashing`]: the source fingerprint computed during the same read (decision G4);
//! - [`sort`]: the external merge sort with bounded memory;
//! - [`primary`]: sort entries of the primary table and the duplicate rule (decision D5);
//! - [`index`]: sort entries of the secondary index tables (decision D2);
//! - [`table`]: the LevelDB table writer (decisions D1 and G2);
//! - [`log`]: the per-run log mirrored to the console (decision G1);
//! - [`messages`]: every user-facing text.

mod abort;
pub mod cli;
pub mod hashing;
pub mod index;
pub mod ingest;
pub mod log;
pub mod messages;
pub mod preflight;
pub mod primary;
mod publish;
pub mod sort;
pub mod table;
pub mod validation;

pub use abort::Abort;
pub use cli::Config;

use std::io;

use crime_dataset::{IndexKind, PRIMARY_FILE_NAME, RecordCounts, SourceInfo, primary_key};

use crate::index::{INDEX_COUNT, IndexEntry, IndexTable};
use crate::ingest::CsvIngest;
use crate::log::RunLog;
use crate::primary::{Dedup, PrimaryEntry, Step};
use crate::publish::Staging;
use crate::sort::{ExternalSort, Sorted};
use crate::table::{TableError, TableStats, TableWriter};

/// What a completed run produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunSummary {
    /// Records in the primary table, and records rejected while reading or as duplicates.
    pub counts: RecordCounts,
    pub source: SourceInfo,
    pub primary: TableStats,
    /// One per index, in [`IndexKind::ALL`](crime_dataset::IndexKind::ALL) order.
    pub indexes: Vec<IndexTable>,
    /// Segments the primary sort wrote to disk; 0 when every record fit in memory.
    pub sort_segments: usize,
    /// Segments the index sort wrote to disk; 0 when every index entry fit in memory.
    pub index_sort_segments: usize,
}

/// Runs the generator with a log that is mirrored to the console (see [`generate`]).
///
/// The log is created first so that every later abort is recorded in it as well as on the
/// console. When the log itself cannot be created, the message only reaches the console.
pub fn run(config: &Config) -> Result<RunSummary, Abort> {
    let mut log =
        preflight::create_log(&config.log, true).inspect_err(|abort| eprintln!("{abort}"))?;
    let result = generate(config, &mut log);
    if let Err(abort) = &result {
        // The console already shows the message even if the log has become unwritable.
        let _ = log.line(&abort.to_string());
    }
    result
}

/// Every stage after the log exists: preflight, header, rows through validation into the
/// external sort, the duplicate rule on the merged stream, the primary table and, from the
/// records it accepts, a second sort into the five index tables, all published together.
pub fn generate(config: &Config, log: &mut RunLog) -> Result<RunSummary, Abort> {
    let input = preflight::open_input(&config.input)?;
    preflight::check_temp_dir(&config.temp_dir)?;
    preflight::check_output(&config.output, config.overwrite)?;
    let temp_error = |error: io::Error| Abort::temp_io(&config.temp_dir, &error);

    let mut ingest = CsvIngest::new(&config.input, input)?;
    let mut sort = ExternalSort::new(&config.temp_dir, config.segment_size, PrimaryEntry::compare)
        .map_err(temp_error)?;
    while let Some(row) = ingest.next_valid(log)? {
        sort.push(PrimaryEntry::from_row(&row))
            .map_err(temp_error)?;
    }
    let read = ingest.counts();
    let source = ingest.finish()?;
    log.line(&messages::reading_completed(read))?;
    log.line(&messages::source_fingerprint(&source))?;

    let sorted = sort.finish().map_err(temp_error)?;
    let sort_segments = sorted.segment_count();
    log.line(&messages::sort_completed(sort_segments))?;

    let staging = Staging::create(&config.output)
        .map_err(|error| Abort::output_io(&config.output, &error))?;
    let mut index_sort = ExternalSort::new(
        &config.temp_dir,
        config.index_segment_size,
        IndexEntry::compare,
    )
    .map_err(temp_error)?;
    let (primary, duplicates) = write_primary(config, sorted, &staging, log, |entry| {
        for index_entry in index::entries_of(entry)? {
            index_sort.push(index_entry).map_err(temp_error)?;
        }
        Ok(())
    })?;

    let index_sorted = index_sort.finish().map_err(temp_error)?;
    let index_sort_segments = index_sorted.segment_count();
    log.line(&messages::index_sort_completed(index_sort_segments))?;
    let indexes = write_indexes(config, index_sorted, &staging)?;
    let index_counts: Vec<(&str, u64)> = indexes
        .iter()
        .map(|table| (table.kind.name(), table.stats.entries))
        .collect();
    log.line(&messages::indexes_generated(&index_counts))?;
    // Checked before publishing, so a defect never reaches the output folder.
    index::check_index_counts(primary.entries, &indexes)?;

    let mut files = vec![PRIMARY_FILE_NAME];
    files.extend(IndexKind::ALL.map(IndexKind::file_name));
    staging
        .publish(&files)
        .map_err(|error| Abort::output_io(&config.output, &error))?;

    let counts = RecordCounts {
        valid: primary.entries,
        rejected: read.rejected + duplicates,
    };
    log.line(&messages::generation_completed(counts))?;
    Ok(RunSummary {
        counts,
        source,
        primary,
        indexes,
        sort_segments,
        index_sort_segments,
    })
}

/// Writes the merged stream into the staged primary table, applying the duplicate rule.
/// Returns the table statistics and the number of duplicates skipped.
///
/// `on_accepted` sees every accepted entry, in key order, right after it is added to the table:
/// the secondary indexes are built from exactly these entries.
fn write_primary(
    config: &Config,
    mut sorted: Sorted<PrimaryEntry>,
    staging: &Staging,
    log: &mut RunLog,
    mut on_accepted: impl FnMut(&PrimaryEntry) -> Result<(), Abort>,
) -> Result<(TableStats, u64), Abort> {
    let table_error = |error| table_abort(config, PRIMARY_FILE_NAME, error);
    let mut table = TableWriter::create(&staging.path(PRIMARY_FILE_NAME), config.compression)
        .map_err(table_error)?;
    let mut duplicates = 0;
    for step in Dedup::new(sorted.by_ref()) {
        match step.map_err(|error| Abort::temp_io(&config.temp_dir, &error))? {
            Step::Accepted(entry) => {
                table
                    .add(&primary_key(entry.id), &entry.value)
                    .map_err(table_error)?;
                on_accepted(&entry)?;
            }
            Step::Duplicate { entry, first_line } => {
                duplicates += 1;
                log.line(&primary::duplicate_log_line(&entry, first_line))?;
            }
        }
    }
    // The segments are no longer needed: free the temporary space before finishing the table.
    sorted
        .close()
        .map_err(|error| Abort::temp_io(&config.temp_dir, &error))?;
    let stats = table.finish().map_err(table_error)?;
    Ok((stats, duplicates))
}

/// Writes one staged table per index from the merged index stream, in [`IndexKind::ALL`] order.
///
/// The stream is ordered by index ordinal first, so the table of an index is finished as soon
/// as an entry of a later index arrives. Every index gets its table, even one without entries.
fn write_indexes(
    config: &Config,
    mut sorted: Sorted<IndexEntry>,
    staging: &Staging,
) -> Result<Vec<IndexTable>, Abort> {
    let temp_error = |error: io::Error| Abort::temp_io(&config.temp_dir, &error);
    let table_error = |kind: IndexKind| move |error| table_abort(config, kind.file_name(), error);
    let create = |kind: IndexKind| {
        TableWriter::create(&staging.path(kind.file_name()), config.compression)
            .map_err(table_error(kind))
    };
    let finish = |kind: IndexKind, table: TableWriter| {
        let stats = table.finish().map_err(table_error(kind))?;
        Ok::<_, Abort>(IndexTable { kind, stats })
    };
    // A later ordinal than every index left, or an earlier one than the current index, means
    // the stream is not in table order.
    let unordered = |kind: IndexKind| Abort::UnorderedKeys {
        file: config.output.join(kind.file_name()),
    };

    let mut tables = Vec::with_capacity(INDEX_COUNT);
    let mut kinds = IndexKind::ALL.into_iter();
    let mut kind = kinds.next().expect("there is at least one index");
    let mut table = create(kind)?;
    for entry in sorted.by_ref() {
        let entry = entry.map_err(temp_error)?;
        if entry.ordinal < kind.ordinal() {
            return Err(unordered(kind));
        }
        while entry.ordinal > kind.ordinal() {
            tables.push(finish(kind, table)?);
            kind = kinds.next().ok_or_else(|| unordered(kind))?;
            table = create(kind)?;
        }
        table.add(&entry.key, &[]).map_err(table_error(kind))?;
    }
    tables.push(finish(kind, table)?);
    for kind in kinds {
        tables.push(finish(kind, create(kind)?)?);
    }
    sorted.close().map_err(temp_error)?;
    Ok(tables)
}

/// The abort for a failure of the table `file_name`.
fn table_abort(config: &Config, file_name: &str, error: TableError) -> Abort {
    match error {
        TableError::OutOfOrder { .. } => Abort::UnorderedKeys {
            file: config.output.join(file_name),
        },
        TableError::Io(error) => Abort::output_io(&config.output, &error),
    }
}
