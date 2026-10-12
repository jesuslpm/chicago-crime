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
//! - [`publish`]: the output lock, leftovers, version folders and the atomic switch of
//!   `CURRENT` (parent flow A, step 8);
//! - [`log`]: the per-run log mirrored to the console (decision G1);
//! - [`messages`]: every user-facing text;
//! - [`fail`]: deterministic failure injection for the tests.

mod abort;
pub mod cli;
pub mod fail;
pub mod hashing;
pub mod index;
pub mod ingest;
pub mod log;
pub mod messages;
pub mod preflight;
pub mod primary;
pub mod publish;
pub mod sort;
pub mod table;
pub mod validation;

pub use abort::Abort;
pub use cli::Config;

use std::io;
use std::path::{Path, PathBuf};

use chrono::SecondsFormat;
use crime_dataset::{
    IndexInfo, IndexKind, Manifest, PRIMARY_FILE_NAME, RecordCounts, SCHEMA_VERSION, SourceInfo,
    TableInfo, primary_key,
};

use crate::fail::FailPoint;
use crate::index::{INDEX_COUNT, IndexEntry, IndexTable};
use crate::ingest::CsvIngest;
use crate::log::RunLog;
use crate::primary::{Dedup, PrimaryEntry, Step};
use crate::publish::OutputFolder;
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
    /// The published version folder, which `CURRENT` now names.
    pub dataset_dir: PathBuf,
}

/// Runs the generator with a log that is mirrored to the console (see [`generate`]).
///
/// The log is created first so that every later abort is recorded in it as well as on the
/// console. When the log itself cannot be created, the messages only reach the console.
pub fn run(config: &Config) -> Result<RunSummary, Abort> {
    let mut log = preflight::create_log(&config.log, true).inspect_err(|abort| {
        eprintln!("{abort}");
        eprintln!("{}", messages::generation_cancelled());
    })?;
    generate(config, &mut log)
}

/// Every stage after the log exists (see [`build`]). An abort is logged with its cause followed
/// by the cancellation line (AC-G9): whatever failed, the published dataset, if any, is intact.
pub fn generate(config: &Config, log: &mut RunLog) -> Result<RunSummary, Abort> {
    let result = build(config, log);
    if let Err(abort) = &result {
        // Best effort: the console shows both lines even if the log has become unwritable.
        let _ = log.line(&abort.to_string());
        let _ = log.line(&messages::generation_cancelled());
    }
    result
}

/// Preflight and the lock of the output folder, header, rows through validation into the
/// external sort, the duplicate rule on the merged stream, the primary table and, from the
/// records it accepts, a second sort into the five index tables, all written into a new version
/// folder that is published with its manifest in one switch.
///
/// Every value that holds a temporary resource (the sorts, the tables, the new version folder,
/// the output lock) cleans it up when dropped, so an early return leaves nothing behind.
fn build(config: &Config, log: &mut RunLog) -> Result<RunSummary, Abort> {
    let input = preflight::open_input(&config.input)?;
    preflight::check_temp_dir(&config.temp_dir)?;
    preflight::check_output(&config.output)?;
    // Held until the run ends, so no other generation can change the folder meanwhile.
    let output = OutputFolder::open(&config.output, config.overwrite, log)?;
    let temp_error = |error: io::Error| Abort::temp_io(&config.temp_dir, &error);
    let output_error = |error: io::Error| Abort::output_io(&config.output, &error);

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
    if read.valid == 0 && read.rejected == 0 {
        log.line(&messages::empty_csv())?;
    }
    log.line(&messages::source_fingerprint(&source))?;

    let sorted = sort.finish().map_err(temp_error)?;
    let sort_segments = sorted.segment_count();
    log.line(&messages::sort_completed(sort_segments))?;

    let version = output.create_version().map_err(output_error)?;
    fail::check(FailPoint::VersionCreated).map_err(output_error)?;
    let mut index_sort = ExternalSort::new(
        &config.temp_dir,
        config.index_segment_size,
        IndexEntry::compare,
    )
    .map_err(temp_error)?;
    let (primary, duplicates) = write_primary(config, sorted, version.path(), log, |entry| {
        for index_entry in index::entries_of(entry)? {
            index_sort.push(index_entry).map_err(temp_error)?;
        }
        fail::check(FailPoint::BothSortsAlive).map_err(temp_error)
    })?;

    let index_sorted = index_sort.finish().map_err(temp_error)?;
    let index_sort_segments = index_sorted.segment_count();
    log.line(&messages::index_sort_completed(index_sort_segments))?;
    let indexes = write_indexes(config, index_sorted, version.path())?;
    let index_counts: Vec<(&str, u64)> = indexes
        .iter()
        .map(|table| (table.kind.name(), table.stats.entries))
        .collect();
    log.line(&messages::indexes_generated(&index_counts))?;
    // Checked before publishing, so a defect never reaches the output folder.
    index::check_index_counts(primary.entries, &indexes)?;
    fail::check(FailPoint::TablesWritten).map_err(output_error)?;

    let counts = RecordCounts {
        valid: primary.entries,
        rejected: read.rejected + duplicates,
    };
    let manifest = manifest(config, &source, counts, primary, &indexes);
    let dataset_dir = output.publish(version, &manifest, log)?;
    // The dataset is published: the run has succeeded even if the log fails from here on.
    let _ = log.line(&messages::generation_completed(counts));
    Ok(RunSummary {
        counts,
        source,
        primary,
        indexes,
        sort_segments,
        index_sort_segments,
        dataset_dir,
    })
}

/// The manifest of a finished run, stamped with the current local time.
fn manifest(
    config: &Config,
    source: &SourceInfo,
    records: RecordCounts,
    primary: TableStats,
    indexes: &[IndexTable],
) -> Manifest {
    Manifest {
        schema_version: SCHEMA_VERSION,
        generated_at: chrono::Local::now().to_rfc3339_opts(SecondsFormat::Secs, false),
        source: source.clone(),
        records,
        compression: config.compression,
        primary: TableInfo {
            file: PRIMARY_FILE_NAME.to_owned(),
            entries: primary.entries,
            size_bytes: primary.size_bytes,
        },
        indexes: indexes
            .iter()
            .map(|table| IndexInfo {
                name: table.kind.name().to_owned(),
                file: table.kind.file_name().to_owned(),
                entries: table.stats.entries,
                size_bytes: table.stats.size_bytes,
            })
            .collect(),
    }
}

/// Writes the merged stream into the primary table in `dir`, applying the duplicate rule.
/// Returns the table statistics and the number of duplicates skipped.
///
/// `on_accepted` sees every accepted entry, in key order, right after it is added to the table:
/// the secondary indexes are built from exactly these entries.
fn write_primary(
    config: &Config,
    mut sorted: Sorted<PrimaryEntry>,
    dir: &Path,
    log: &mut RunLog,
    mut on_accepted: impl FnMut(&PrimaryEntry) -> Result<(), Abort>,
) -> Result<(TableStats, u64), Abort> {
    let path = dir.join(PRIMARY_FILE_NAME);
    let table_error = |error| table_abort(config, &path, error);
    let mut table = TableWriter::create(&path, config.compression).map_err(table_error)?;
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

/// Writes one table per index into `dir` from the merged index stream, in [`IndexKind::ALL`]
/// order.
///
/// The stream is ordered by index ordinal first, so the table of an index is finished as soon
/// as an entry of a later index arrives. Every index gets its table, even one without entries.
fn write_indexes(
    config: &Config,
    mut sorted: Sorted<IndexEntry>,
    dir: &Path,
) -> Result<Vec<IndexTable>, Abort> {
    let temp_error = |error: io::Error| Abort::temp_io(&config.temp_dir, &error);
    let table_error =
        |kind: IndexKind| move |error| table_abort(config, &dir.join(kind.file_name()), error);
    let create = |kind: IndexKind| {
        TableWriter::create(&dir.join(kind.file_name()), config.compression)
            .map_err(table_error(kind))
    };
    let finish = |kind: IndexKind, table: TableWriter| {
        let stats = table.finish().map_err(table_error(kind))?;
        Ok::<_, Abort>(IndexTable { kind, stats })
    };
    // A later ordinal than every index left, or an earlier one than the current index, means
    // the stream is not in table order.
    let unordered = |kind: IndexKind| Abort::UnorderedKeys {
        file: dir.join(kind.file_name()),
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

/// The abort for a failure of the table at `path`.
fn table_abort(config: &Config, path: &Path, error: TableError) -> Abort {
    match error {
        TableError::OutOfOrder { .. } => Abort::UnorderedKeys {
            file: path.to_path_buf(),
        },
        TableError::Io(error) => Abort::output_io(&config.output, &error),
    }
}

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;
    use std::num::NonZeroUsize;

    use crime_dataset::Compression;

    use super::*;

    /// A merged index stream that yields `entries` exactly in the given order: the sort is
    /// stable and this order calls every pair equal.
    fn stream_in_this_order(temp_dir: &Path, entries: &[(u8, &[u8])]) -> Sorted<IndexEntry> {
        let keep_order = |_: &IndexEntry, _: &IndexEntry| Ordering::Equal;
        let size = NonZeroUsize::new(1_000).expect("positive");
        let mut sort = ExternalSort::new(temp_dir, size, keep_order).expect("the sort can start");
        for &(ordinal, key) in entries {
            sort.push(IndexEntry {
                ordinal,
                key: key.to_vec(),
            })
            .expect("the entry can be pushed");
        }
        sort.finish().expect("the sort can finish")
    }

    fn config(dir: &Path) -> Config {
        Config {
            input: dir.join("input.csv"),
            output: dir.join("output"),
            temp_dir: dir.to_path_buf(),
            log: dir.join("run.log"),
            overwrite: false,
            compression: Compression::None,
            segment_size: NonZeroUsize::new(10).expect("positive"),
            index_segment_size: NonZeroUsize::new(10).expect("positive"),
        }
    }

    #[test]
    fn an_entry_of_an_earlier_index_than_the_current_one_is_refused() {
        let dir = tempfile::tempdir().expect("a temporary directory can be created");
        let primary_type = IndexKind::PrimaryType.ordinal();
        let date = IndexKind::Date.ordinal();
        let sorted = stream_in_this_order(dir.path(), &[(primary_type, b"b"), (date, b"a")]);

        let abort = write_indexes(&config(dir.path()), sorted, dir.path())
            .expect_err("the date entry arrives after the primary type table started");

        assert_eq!(
            abort,
            Abort::UnorderedKeys {
                file: dir.path().join(IndexKind::PrimaryType.file_name())
            }
        );
    }

    #[test]
    fn an_ordinal_after_the_last_index_is_refused() {
        let dir = tempfile::tempdir().expect("a temporary directory can be created");
        let date = IndexKind::Date.ordinal();
        let sorted = stream_in_this_order(dir.path(), &[(date, b"a"), (u8::MAX, b"b")]);

        let abort = write_indexes(&config(dir.path()), sorted, dir.path())
            .expect_err("no index has that ordinal");

        assert_eq!(
            abort,
            Abort::UnorderedKeys {
                file: dir.path().join(IndexKind::CaseNumber.file_name())
            }
        );
    }
}
