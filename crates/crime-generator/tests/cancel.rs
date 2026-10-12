//! Cancellation by the operator (child flow step 4, AC-G7) and the exit code of every outcome
//! (decision G3).
//!
//! Ctrl+C only raises the cancellation flag. The tests raise it deterministically at each place
//! where the run checks it, through the `Cancel...` fail points of `crime_generator::fail`, which
//! `cargo test` enables for this crate (feature `fail-points`).

mod common;

use std::num::NonZeroUsize;

use common::{
    Scenario, distinct_row, folder_entries, generate, generate_ok, generate_with, published_ids,
    rerun, snapshot,
};
use crime_generator::cancel::CancelFlag;
use crime_generator::fail::{FailPoint, arm, arm_after};
use crime_generator::{Abort, Config, RunSummary, exit_code, exit_status};

const OPERATOR_CANCELLED: &str =
    "Generación cancelada por el operador. No se ha modificado el conjunto existente.";
const GENERIC_CANCELLED: &str = "Generación cancelada. No se ha modificado el conjunto existente.";

fn segment_size(size: usize) -> NonZeroUsize {
    NonZeroUsize::new(size).expect("a positive segment size")
}

/// The run was cancelled: the abort, its exact message as the last and only abort line of the
/// log, without the generic cancellation line, and the exit code of a cancellation.
fn assert_cancelled(result: &Result<RunSummary, Abort>, log: &str) {
    let abort = result.as_ref().expect_err("the run is cancelled");
    assert_eq!(*abort, Abort::Cancelled);
    assert_eq!(abort.to_string(), OPERATOR_CANCELLED);
    assert_eq!(
        log.lines().last(),
        Some(OPERATOR_CANCELLED),
        "the log ends with the cancellation:\n{log}"
    );
    assert_eq!(
        log.matches(OPERATOR_CANCELLED).count(),
        1,
        "said once:\n{log}"
    );
    assert!(
        !log.contains(GENERIC_CANCELLED),
        "no generic cancellation line:\n{log}"
    );
    assert!(!log.contains("Generación completada"), "{log}");
    assert_eq!(exit_status(result), exit_code::CANCELLED);
}

/// Publishes IDs 1 to 6, then cancels a run that would replace them with IDs 11 to 16: the flag is
/// raised the `passes + 1`-th time the run reaches `point`. Small segments make both sorts use the
/// disk. The previous dataset must stay byte-identical, no new version folder, `CURRENT.tmp` or
/// sort folder may be left, and the next run must complete normally. Returns the cancelled log.
fn cancel_a_replacement_at(point: FailPoint, passes: u32) -> String {
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
        let _armed = arm_after(point, passes);
        generate(&rerun(&config, "cancelled", true))
    };

    assert_cancelled(&result, &log);
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
        !next.contains("Se han descartado restos"),
        "{point:?}: the cancelled run left nothing behind: {next}"
    );
    assert_eq!(published_ids(&config.output), [11, 12, 13, 14, 15, 16]);
    log
}

#[test]
fn a_cancellation_while_rows_are_read_removes_the_sort_folder_and_keeps_the_previous_dataset() {
    // The fifth row: four rows already went into two segments on disk.
    let log = cancel_a_replacement_at(FailPoint::CancelInRows, 4);

    assert!(!log.contains("Lectura completada"), "{log}");
}

#[test]
fn a_cancellation_during_the_primary_merge_removes_both_sort_folders_and_the_new_version() {
    // The third merged entry: the index sort already holds entries of the first two.
    let log = cancel_a_replacement_at(FailPoint::CancelInPrimaryMerge, 2);

    assert!(
        log.contains("Ordenación completada. Segmentos temporales en disco: 3."),
        "the primary merge was reading segments from disk: {log}"
    );
    assert!(
        !log.contains("Ordenación de los índices completada"),
        "{log}"
    );
}

#[test]
fn a_cancellation_while_the_index_tables_are_written_removes_the_new_version() {
    // The eleventh index entry: the date table is complete and the next one has started.
    let log = cancel_a_replacement_at(FailPoint::CancelInIndexWrite, 10);

    assert!(
        log.contains("Ordenación de los índices completada"),
        "the index tables were being written: {log}"
    );
    assert!(!log.contains("Índices generados"), "{log}");
}

#[test]
fn a_cancellation_just_before_the_switch_removes_the_temporary_current_and_the_new_version() {
    let log = cancel_a_replacement_at(FailPoint::CancelBeforeSwitch, 0);

    assert!(
        log.contains("Índices generados"),
        "every table was written: {log}"
    );
}

#[test]
fn a_flag_raised_before_the_first_row_cancels_the_run_and_removes_the_output_folder_it_created() {
    let scenario = Scenario::new(&[distinct_row(1), distinct_row(2)]);
    let config = scenario.config("dataset");
    let cancel = CancelFlag::new();
    cancel.raise();

    let (result, log) = generate_with(&config, &cancel);

    assert_cancelled(&result, &log);
    assert_eq!(log.lines().collect::<Vec<_>>(), [OPERATOR_CANCELLED]);
    assert!(
        !config.output.exists(),
        "the run created it and leaves nothing"
    );
    assert_eq!(folder_entries(&scenario.temp_dir), Vec::<String>::new());
}

#[test]
fn clones_share_the_flag() {
    let flag = CancelFlag::new();
    let handler_copy = flag.clone();
    assert!(!flag.is_raised());

    handler_copy.raise();

    assert!(flag.is_raised());
}

#[test]
fn a_completed_run_exits_with_zero_and_every_abort_with_its_own_non_zero_code() {
    let scenario = Scenario::new(&[distinct_row(1)]);
    let completed = generate(&scenario.config("dataset")).0;
    let failed = {
        let _armed = arm(FailPoint::TablesWritten);
        generate(&scenario.config("failed")).0
    };

    assert_eq!(exit_status(&completed), 0);
    assert_eq!(exit_status(&failed), exit_code::ABORTED);
    assert_eq!(exit_status(&Err(Abort::DatasetExists)), exit_code::ABORTED);
    assert_eq!(exit_status(&Err(Abort::Cancelled)), exit_code::CANCELLED);
    let codes = [
        exit_code::COMPLETED,
        exit_code::ABORTED,
        exit_code::USAGE,
        exit_code::CANCELLED,
    ];
    assert_eq!(codes[0], 0);
    for (index, code) in codes.iter().enumerate() {
        assert!(
            !codes[..index].contains(code),
            "every outcome has its own code: {codes:?}"
        );
    }
}
