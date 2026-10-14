//! Publication of the dataset (parent flow A steps 7 and 8; AC-G6, AC-G8 and AC-G9): a complete
//! version folder named by `CURRENT`, the manifest, refusing and replacing an existing dataset,
//! the output lock, leftovers of interrupted runs, foreign files and the CSV without records.
//!
//! Failures are injected deterministically through the fail points of `crime_generator::fail`,
//! which `cargo test` enables for this crate (feature `fail-points`).

mod common;

use std::fs;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};

use common::{
    DATASET_FILES, Scenario, dataset_dir, distinct_row, folder_entries, generate, generate_ok,
    published_ids, read_table, rerun, sample_row, snapshot, with_field,
};
use crime_dataset::{
    Compression, IndexKind, Manifest, RecordCounts, is_version_dir_name, resolve_current,
};
use crime_generator::cli::local_timestamp;
use crime_generator::fail::{FailPoint, arm};
use crime_generator::messages::{leftover_kept, previous_version_kept};
use crime_generator::publish::{OutputLock, version_dir_name};
use crime_generator::{Abort, Config};
use sha2::{Digest, Sha256};

const DATASET_EXISTS: &str = "La carpeta de salida ya contiene un conjunto de datos. Usa la \
                              opción de sobrescritura para reemplazarlo.";
const CANCELLED: &str = "Generación cancelada. No se ha modificado el conjunto existente.";
const LEFTOVERS: &str = "Se han descartado restos de una generación interrumpida anterior.";
const EMPTY_CSV: &str = "Aviso: el CSV no contiene registros; el conjunto generado está vacío.";

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn segment_size(size: usize) -> NonZeroUsize {
    NonZeroUsize::new(size).expect("a positive segment size")
}

fn published_manifest(output: &Path) -> Manifest {
    let text = fs::read_to_string(dataset_dir(output).join("manifest.json"))
        .expect("the manifest can be read");
    Manifest::from_json(&text).unwrap_or_else(|error| panic!("{error}: {text}"))
}

/// Name of the version folder that `CURRENT` names.
fn version_name(output: &Path) -> String {
    dataset_dir(output)
        .file_name()
        .and_then(|name| name.to_str())
        .expect("a version folder name")
        .to_owned()
}

/// Creates what a run killed before its switch leaves behind: a version folder named `name` that
/// nothing names, with a partial table.
fn crashed_version(output: &Path, name: &str) -> PathBuf {
    let crashed = output.join(name);
    fs::create_dir_all(&crashed).expect("the fixture can be created");
    fs::write(crashed.join("primary.sst"), b"partial table").expect("the fixture can be written");
    crashed
}

/// Creates folders a user may keep in the output folder whose names start like a version folder
/// but do not have its shape, each with a file in it.
fn user_folders_with_the_version_prefix(output: &Path) {
    for (folder, file) in [
        ("dataset-backup", "copy.csv"),
        ("dataset-2024", "notes.txt"),
        ("dataset-20261012", "report.pdf"),
    ] {
        let folder = output.join(folder);
        fs::create_dir_all(&folder).expect("the fixture can be created");
        fs::write(folder.join(file), b"mine").expect("the fixture can be written");
    }
}

/// The log of an aborted run ends with the cause and then the cancellation line (AC-G9).
fn assert_log_explains(log: &str, abort: &Abort) {
    let cause = abort.to_string();
    let lines: Vec<&str> = log.lines().collect();
    assert!(
        lines.ends_with(&[cause.as_str(), CANCELLED]),
        "the log should end with {cause:?} and {CANCELLED:?}:\n{log}"
    );
}

#[test]
fn the_first_run_publishes_current_and_one_version_folder_with_the_tables_and_the_manifest() {
    let rows = [
        distinct_row(30),
        with_field(sample_row(), "Year", b"unknown"),
        distinct_row(10),
        distinct_row(20),
    ];
    let scenario = Scenario::new(&rows);
    let config = scenario.config("dataset");

    let (summary, log) = generate_ok(&config);

    let version = version_name(&config.output);
    assert!(version.starts_with("dataset-"), "{version}");
    assert_eq!(
        folder_entries(&config.output),
        ["CURRENT".to_owned(), version.clone()],
        "CURRENT and one version folder, nothing else"
    );
    assert_eq!(
        fs::read_to_string(config.output.join("CURRENT")).expect("CURRENT can be read"),
        format!("{version}\n")
    );
    let dataset = config.output.join(&version);
    assert_eq!(
        resolve_current(&config.output).expect("the dataset contract resolves CURRENT"),
        dataset
    );
    assert_eq!(summary.dataset_dir, dataset);
    assert_eq!(folder_entries(&dataset), DATASET_FILES);
    assert_eq!(folder_entries(&scenario.temp_dir), Vec::<String>::new());
    assert_eq!(published_ids(&config.output), [10, 20, 30]);
    assert!(!log.contains(LEFTOVERS), "{log}");
    assert!(!log.contains(EMPTY_CSV), "{log}");

    let manifest = published_manifest(&config.output);
    let input = fs::read(&scenario.input).expect("the input can be read");
    let size = |file: &str| fs::metadata(dataset.join(file)).expect("metadata").len();
    assert_eq!(summary.manifest_size_bytes, size("manifest.json"));
    assert_eq!(summary.log_path, config.log);
    assert_eq!(manifest.schema_version, 2);
    chrono::DateTime::parse_from_rfc3339(&manifest.generated_at)
        .unwrap_or_else(|error| panic!("{:?} is ISO 8601: {error}", manifest.generated_at));
    assert_eq!(manifest.source.file_name, "crimes.csv");
    assert_eq!(manifest.source.size_bytes, input.len() as u64);
    assert_eq!(manifest.source.sha256, sha256_hex(&input));
    assert_eq!(manifest.source, summary.source);
    assert_eq!(
        manifest.records,
        RecordCounts {
            valid: 3,
            rejected: 1
        }
    );
    assert_eq!(manifest.compression, Compression::Snappy);
    assert_eq!(
        (
            manifest.primary.file.as_str(),
            manifest.primary.entries,
            manifest.primary.size_bytes
        ),
        ("primary.sst", 3, size("primary.sst"))
    );
    let indexes: Vec<(String, String, u64, u64)> = manifest
        .indexes
        .iter()
        .map(|index| {
            (
                index.name.clone(),
                index.file.clone(),
                index.entries,
                index.size_bytes,
            )
        })
        .collect();
    let expected: Vec<(String, String, u64, u64)> = IndexKind::ALL
        .iter()
        .map(|kind| {
            (
                kind.name().to_owned(),
                kind.file_name().to_owned(),
                3,
                size(kind.file_name()),
            )
        })
        .collect();
    assert_eq!(indexes, expected);
}

#[test]
fn the_manifest_records_disabled_compression() {
    let scenario = Scenario::new(&[distinct_row(1)]);
    let config = Config {
        compression: Compression::None,
        ..scenario.config("dataset")
    };

    generate_ok(&config);

    assert_eq!(
        published_manifest(&config.output).compression,
        Compression::None
    );
}

#[test]
fn a_second_run_without_overwrite_is_refused_before_reading_rows_and_changes_nothing() {
    let scenario = Scenario::new(&[distinct_row(1), distinct_row(2)]);
    let config = scenario.config("dataset");
    generate_ok(&config);
    let published = snapshot(&config.output);
    scenario.rewrite_input(&[7, 8, 9]);

    let (result, log) = generate(&rerun(&config, "second", false));

    let abort = result.expect_err("the existing dataset needs the overwrite option");
    assert_eq!(abort, Abort::DatasetExists);
    assert_eq!(abort.to_string(), DATASET_EXISTS);
    assert_eq!(
        log.lines().collect::<Vec<_>>(),
        [DATASET_EXISTS, CANCELLED],
        "refused before reading any row"
    );
    assert_eq!(
        snapshot(&config.output),
        published,
        "nothing changed, byte for byte"
    );
    assert_eq!(folder_entries(&scenario.temp_dir), Vec::<String>::new());
}

#[test]
fn overwrite_publishes_a_new_version_and_removes_the_previous_one() {
    let scenario = Scenario::new(&[distinct_row(1), distinct_row(2), distinct_row(3)]);
    let config = scenario.config("dataset");
    generate_ok(&config);
    let previous = version_name(&config.output);
    scenario.rewrite_input(&[20, 10]);

    let (summary, log) = generate_ok(&rerun(&config, "second", true));

    let current = version_name(&config.output);
    assert_ne!(current, previous, "a new version folder");
    assert_eq!(
        folder_entries(&config.output),
        ["CURRENT".to_owned(), current.clone()],
        "the previous version folder is gone"
    );
    assert_eq!(summary.dataset_dir, config.output.join(&current));
    assert_eq!(published_ids(&config.output), [10, 20]);
    let manifest = published_manifest(&config.output);
    assert_eq!(
        manifest.records,
        RecordCounts {
            valid: 2,
            rejected: 0
        }
    );
    for kind in IndexKind::ALL {
        let entries = read_table(&dataset_dir(&config.output).join(kind.file_name())).len();
        assert_eq!(entries, 2, "{}", kind.name());
    }
    assert!(
        !log.contains(LEFTOVERS),
        "a replaced version is not a leftover: {log}"
    );
    assert_eq!(folder_entries(&scenario.temp_dir), Vec::<String>::new());
}

/// Publishes IDs 1 to 6, then fails a run that would replace them with IDs 11 to 16 at `point`,
/// with small segments so that both sorts use the disk. The previous dataset must stay
/// byte-identical, no new folder or temporary file may be left in the output folder or the
/// temporary folder, and the next run must complete normally (AC-G6). Returns the failed log.
fn fail_a_replacement_at(point: FailPoint) -> String {
    let rows: Vec<_> = (1..=6).map(distinct_row).collect();
    let scenario = Scenario::new(&rows);
    let config = Config {
        segment_size: segment_size(2),
        index_segment_size: segment_size(3),
        ..scenario.config("dataset")
    };
    generate_ok(&config);
    let published = snapshot(&config.output);
    scenario.rewrite_input(&[11, 12, 13, 14, 15, 16]);

    let (result, log) = {
        let _armed = arm(point);
        generate(&rerun(&config, "failed", true))
    };

    let abort = result.expect_err("the injected failure aborts the run");
    assert_log_explains(&log, &abort);
    assert_eq!(
        snapshot(&config.output),
        published,
        "{point:?}: CURRENT and the previous version are byte-identical, and nothing else exists"
    );
    assert_eq!(
        folder_entries(&scenario.temp_dir),
        Vec::<String>::new(),
        "{point:?}: no sort folder is left"
    );

    let (_, next) = generate_ok(&rerun(&config, "next", true));
    assert!(
        !next.contains(LEFTOVERS),
        "{point:?}: the failed run left nothing behind: {next}"
    );
    assert_eq!(published_ids(&config.output), [11, 12, 13, 14, 15, 16]);
    log
}

#[test]
fn a_failure_after_the_version_folder_exists_publishes_nothing() {
    let log = fail_a_replacement_at(FailPoint::VersionCreated);

    assert!(!log.contains("Índices generados"), "{log}");
}

#[test]
fn an_abort_while_both_sorts_are_alive_removes_both_temporary_folders_and_the_new_version() {
    let log = fail_a_replacement_at(FailPoint::BothSortsAlive);

    assert!(
        log.contains("Ordenación completada. Segmentos temporales en disco: 3."),
        "the primary merge was reading segments from disk: {log}"
    );
    assert!(!log.contains("Índices generados"), "{log}");
}

#[test]
fn a_failure_after_the_tables_are_written_keeps_the_previous_dataset() {
    let log = fail_a_replacement_at(FailPoint::TablesWritten);

    assert!(
        log.contains("Índices generados"),
        "the tables were written: {log}"
    );
}

#[test]
fn a_failure_just_before_the_switch_keeps_the_previous_dataset() {
    let log = fail_a_replacement_at(FailPoint::BeforeSwitch);

    assert!(log.contains("Índices generados"), "{log}");
}

#[test]
fn a_failed_first_run_removes_the_output_folder_it_created() {
    let scenario = Scenario::new(&[distinct_row(1)]);
    let config = scenario.config("dataset");

    let (result, log) = {
        let _armed = arm(FailPoint::TablesWritten);
        generate(&config)
    };

    let abort = result.expect_err("the injected failure aborts the run");
    assert_log_explains(&log, &abort);
    assert!(
        !config.output.exists(),
        "the run created the output folder and leaves nothing behind"
    );
    assert_eq!(folder_entries(&scenario.temp_dir), Vec::<String>::new());
}

/// Builds before the Linux support PRD left their lock file in the output folder on Unix; now the
/// folder itself is locked, so that file is a leftover like any other.
#[cfg(not(windows))]
#[test]
fn a_lock_file_left_by_an_older_unix_build_is_discarded_as_a_leftover() {
    let scenario = Scenario::new(&[distinct_row(1)]);
    let config = scenario.config("dataset");
    fs::create_dir_all(&config.output).expect("the fixture can be created");
    fs::write(config.output.join(".generator.lock"), b"").expect("the fixture can be written");

    let (_, log) = generate_ok(&config);

    assert_eq!(log.matches(LEFTOVERS).count(), 1, "{log}");
    let version = version_name(&config.output);
    assert_eq!(
        folder_entries(&config.output),
        ["CURRENT".to_owned(), version]
    );
}

#[test]
fn leftovers_of_an_interrupted_run_are_discarded_with_one_message() {
    let scenario = Scenario::new(&[distinct_row(1)]);
    let config = scenario.config("dataset");
    // What a run killed before its switch leaves behind: a version folder that nothing names,
    // with a partial table, and a temporary CURRENT.
    let crashed = config.output.join("dataset-20200101-000000-999");
    fs::create_dir_all(&crashed).expect("the fixture can be created");
    fs::write(crashed.join("primary.sst"), b"partial table").expect("the fixture can be written");
    fs::write(
        config.output.join("CURRENT.tmp"),
        b"dataset-20200101-000000-999\n",
    )
    .expect("the fixture can be written");

    let (_, log) = generate_ok(&config);

    assert_eq!(
        log.matches(LEFTOVERS).count(),
        1,
        "one message per run: {log}"
    );
    let version = version_name(&config.output);
    assert_eq!(
        folder_entries(&config.output),
        ["CURRENT".to_owned(), version]
    );
    assert_eq!(published_ids(&config.output), [1]);
}

#[test]
fn leftovers_next_to_a_dataset_survive_a_refused_run_and_are_discarded_by_an_overwrite() {
    let scenario = Scenario::new(&[distinct_row(1)]);
    let config = scenario.config("dataset");
    generate_ok(&config);
    let crashed = config.output.join("dataset-20200101-000000-999");
    fs::create_dir(&crashed).expect("the fixture can be created");
    fs::write(crashed.join("index-date.sst"), b"partial").expect("the fixture can be written");
    let before = snapshot(&config.output);

    let (result, log) = generate(&rerun(&config, "refused", false));

    assert_eq!(result.expect_err("refused"), Abort::DatasetExists);
    assert_eq!(
        snapshot(&config.output),
        before,
        "a refused run changes nothing"
    );
    assert!(!log.contains(LEFTOVERS), "{log}");

    let (_, log) = generate_ok(&rerun(&config, "overwrite", true));

    assert_eq!(log.matches(LEFTOVERS).count(), 1, "{log}");
    assert!(!crashed.exists());
    let version = version_name(&config.output);
    assert_eq!(
        folder_entries(&config.output),
        ["CURRENT".to_owned(), version]
    );
}

#[test]
fn a_previous_version_that_cannot_be_deleted_is_left_and_discarded_by_the_next_run() {
    let scenario = Scenario::new(&[distinct_row(1)]);
    let config = scenario.config("dataset");
    generate_ok(&config);
    let first = version_name(&config.output);

    let (_, log) = {
        let _armed = arm(FailPoint::PreviousVersionRemoval);
        generate_ok(&rerun(&config, "second", true))
    };

    let second = version_name(&config.output);
    assert_ne!(first, second);
    assert!(
        log.contains(&previous_version_kept(&config.output.join(&first))),
        "{log}"
    );
    assert!(
        log.contains("Generación completada"),
        "the run still succeeds: {log}"
    );
    let mut expected = vec!["CURRENT".to_owned(), first, second];
    expected.sort();
    assert_eq!(folder_entries(&config.output), expected);

    let (_, log) = generate_ok(&rerun(&config, "third", true));

    assert_eq!(log.matches(LEFTOVERS).count(), 1, "{log}");
    let third = version_name(&config.output);
    assert_eq!(
        folder_entries(&config.output),
        ["CURRENT".to_owned(), third]
    );
}

#[test]
fn a_second_generator_on_the_same_output_is_refused_while_the_lock_is_held() {
    let scenario = Scenario::new(&[distinct_row(1)]);
    let config = scenario.config("dataset");
    generate_ok(&config);
    let published = snapshot(&config.output);
    let lock = OutputLock::acquire(&config.output).expect("the lock is free between runs");

    let (result, log) = generate(&rerun(&config, "concurrent", true));

    let abort = result.expect_err("another generation holds the output folder");
    assert_eq!(
        abort,
        Abort::OutputLocked {
            dir: config.output.clone()
        }
    );
    assert_eq!(
        abort.to_string(),
        format!(
            "Otra generación está usando la carpeta de salida {}.",
            config.output.display()
        )
    );
    assert_log_explains(&log, &abort);
    drop(lock);
    assert_eq!(snapshot(&config.output), published, "nothing changed");

    let (_, log) = generate_ok(&rerun(&config, "after", true));
    assert!(log.contains("Generación completada"), "{log}");
}

#[test]
fn the_lock_is_refused_to_a_second_holder_and_free_again_once_released() {
    let dir = tempfile::tempdir().expect("a temporary directory can be created");

    let first = OutputLock::acquire(dir.path()).expect("the lock is free");
    assert!(
        OutputLock::acquire(dir.path()).is_err(),
        "a second holder is refused"
    );
    drop(first);

    OutputLock::acquire(dir.path()).expect("the lock is free again");
}

#[test]
fn a_csv_with_only_the_header_publishes_an_empty_dataset_with_a_warning() {
    let scenario = Scenario::new(&[]);
    let config = scenario.config("dataset");

    let (summary, log) = generate_ok(&config);

    assert!(log.lines().any(|line| line == EMPTY_CSV), "{log}");
    let empty = RecordCounts {
        valid: 0,
        rejected: 0,
    };
    assert_eq!(summary.counts, empty);
    let dataset = dataset_dir(&config.output);
    assert_eq!(folder_entries(&dataset), DATASET_FILES);
    for file in DATASET_FILES.iter().filter(|file| file.ends_with(".sst")) {
        assert!(read_table(&dataset.join(file)).is_empty(), "{file}");
    }
    let manifest = published_manifest(&config.output);
    assert_eq!(manifest.records, empty);
    assert_eq!(manifest.primary.entries, 0);
    assert!(
        manifest.indexes.iter().all(|index| index.entries == 0),
        "{manifest:?}"
    );
}

#[test]
fn foreign_files_in_the_output_folder_are_refused_and_left_untouched() {
    let scenario = Scenario::new(&[distinct_row(1)]);
    // Even with the overwrite option, which only ever replaces a dataset.
    let config = Config {
        overwrite: true,
        ..scenario.config("dataset")
    };
    let photos = config.output.join("photos");
    fs::create_dir_all(&photos).expect("the fixture can be created");
    fs::write(photos.join("a.jpg"), b"jpeg").expect("the fixture can be written");
    fs::write(config.output.join("notes.txt"), b"keep me").expect("the fixture can be written");
    // A table directly in the output folder (an older layout) is not a dataset either.
    fs::write(config.output.join("primary.sst"), b"old table").expect("the fixture can be written");
    let before = snapshot(&config.output);

    let (result, log) = generate(&config);

    let abort = result.expect_err("foreign files are never replaced");
    assert_eq!(
        abort,
        Abort::ForeignContent {
            dir: config.output.clone(),
            entry: "notes.txt".to_owned()
        }
    );
    assert_log_explains(&log, &abort);
    assert!(!log.contains("Lectura completada"), "{log}");
    assert_eq!(snapshot(&config.output), before, "nothing was touched");
}

#[test]
fn a_foreign_file_next_to_a_dataset_blocks_the_overwrite() {
    let scenario = Scenario::new(&[distinct_row(1)]);
    let config = scenario.config("dataset");
    generate_ok(&config);
    fs::write(config.output.join("readme.md"), b"mine").expect("the fixture can be written");
    let before = snapshot(&config.output);

    let (result, _) = generate(&rerun(&config, "overwrite", true));

    assert_eq!(
        result.expect_err("a foreign file is never deleted"),
        Abort::ForeignContent {
            dir: config.output.clone(),
            entry: "readme.md".to_owned()
        }
    );
    assert_eq!(snapshot(&config.output), before);
}

#[test]
fn folders_that_only_look_like_versions_are_refused_and_never_deleted() {
    let scenario = Scenario::new(&[distinct_row(1)]);
    let config = scenario.config("dataset");
    user_folders_with_the_version_prefix(&config.output);
    let before = snapshot(&config.output);

    let (result, log) = generate(&config);

    let abort = result.expect_err("a user folder is neither a dataset nor a leftover");
    assert_eq!(
        abort,
        Abort::ForeignContent {
            dir: config.output.clone(),
            entry: "dataset-2024".to_owned()
        }
    );
    assert_log_explains(&log, &abort);
    assert!(!log.contains(LEFTOVERS), "{log}");
    assert_eq!(snapshot(&config.output), before, "nothing was touched");
}

#[test]
fn folders_that_only_look_like_versions_block_an_overwrite_and_are_never_deleted() {
    let scenario = Scenario::new(&[distinct_row(1)]);
    let config = scenario.config("dataset");
    generate_ok(&config);
    user_folders_with_the_version_prefix(&config.output);
    let before = snapshot(&config.output);

    let (result, log) = generate(&rerun(&config, "overwrite", true));

    let abort = result.expect_err("a user folder is never deleted, even by an overwrite");
    assert_eq!(
        abort,
        Abort::ForeignContent {
            dir: config.output.clone(),
            entry: "dataset-2024".to_owned()
        }
    );
    assert_log_explains(&log, &abort);
    assert!(!log.contains(LEFTOVERS), "{log}");
    assert_eq!(snapshot(&config.output), before, "nothing was touched");
}

#[test]
fn the_generator_names_version_folders_as_the_dataset_contract_expects() {
    assert_eq!(
        version_dir_name("20261012-101500", 4242, 0),
        "dataset-20261012-101500-4242"
    );
    assert_eq!(
        version_dir_name("20261012-101500", 4242, 3),
        "dataset-20261012-101500-4242-3"
    );
    let timestamp = local_timestamp();
    for (process_id, attempt) in [(std::process::id(), 0), (1, 1), (u32::MAX, 100)] {
        let name = version_dir_name(&timestamp, process_id, attempt);
        assert!(is_version_dir_name(&name), "{name:?} is a version folder");
    }
}

#[test]
fn a_leftover_that_cannot_be_deleted_is_reported_and_kept_for_the_next_run() {
    let scenario = Scenario::new(&[distinct_row(1)]);
    let config = scenario.config("dataset");
    let leftover = "dataset-20200101-000000-999";
    let crashed = crashed_version(&config.output, leftover);

    let (_, log) = {
        let _armed = arm(FailPoint::LeftoverRemoval);
        generate_ok(&config)
    };

    assert!(!log.contains(LEFTOVERS), "nothing was discarded: {log}");
    assert_eq!(
        log.matches(&leftover_kept(&crashed)).count(),
        1,
        "the leftover is named once: {log}"
    );
    assert!(
        log.contains("Generación completada"),
        "the run still publishes: {log}"
    );
    let version = version_name(&config.output);
    assert_ne!(version, leftover);
    let mut expected = vec!["CURRENT".to_owned(), leftover.to_owned(), version];
    expected.sort();
    assert_eq!(folder_entries(&config.output), expected);
    assert_eq!(
        fs::read(crashed.join("primary.sst")).expect("the leftover is intact"),
        b"partial table"
    );
    assert_eq!(published_ids(&config.output), [1]);

    let (_, log) = generate_ok(&rerun(&config, "next", true));

    assert_eq!(log.matches(LEFTOVERS).count(), 1, "{log}");
    assert!(!crashed.exists(), "the next run discards it");
}

#[test]
fn only_leftovers_actually_deleted_are_reported_as_discarded() {
    let scenario = Scenario::new(&[distinct_row(1)]);
    let config = scenario.config("dataset");
    // Leftovers are deleted in name order, so the fail point hits the first one.
    let kept = crashed_version(&config.output, "dataset-20200101-000000-998");
    let deleted = crashed_version(&config.output, "dataset-20200101-000000-999");

    let (_, log) = {
        let _armed = arm(FailPoint::LeftoverRemoval);
        generate_ok(&config)
    };

    assert_eq!(
        log.matches(LEFTOVERS).count(),
        1,
        "one message per run: {log}"
    );
    assert_eq!(log.matches(&leftover_kept(&kept)).count(), 1, "{log}");
    assert!(!log.contains(&leftover_kept(&deleted)), "{log}");
    assert!(kept.exists());
    assert!(!deleted.exists());
}

#[test]
fn a_current_that_names_no_version_is_still_a_dataset_to_refuse_or_replace() {
    let scenario = Scenario::new(&[distinct_row(1)]);
    let config = scenario.config("dataset");
    fs::create_dir(&config.output).expect("the output folder can be created");
    fs::write(config.output.join("CURRENT"), b"dataset-gone\n").expect("the fixture");

    let (result, _) = generate(&config);
    assert_eq!(result.expect_err("refused"), Abort::DatasetExists);

    generate_ok(&rerun(&config, "overwrite", true));
    assert_eq!(published_ids(&config.output), [1]);
    assert_eq!(
        folder_entries(&config.output).len(),
        2,
        "CURRENT and one version"
    );
}
