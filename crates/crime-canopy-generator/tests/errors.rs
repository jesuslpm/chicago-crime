//! Failures of the engine (decision KG7, AC-KG7): the message of the original generator for the
//! folder, one detail line in the log right before it, exit code 1 and the previous dataset
//! intact. The faults are injected with the `fail-points` feature of this crate.

mod common;

use common::{Scenario, dataset_dir, generate, generate_ok, read_canopy, rerun, snapshot};
use crime_canopy_generator::fault::{Fault, arm};
use crime_generator::exit_status_of;

const IDS: [i64; 6] = [5, 3, 9, 1, 7, 2];

/// The lines of `log` from the detail line on.
fn tail_from_detail(log: &str) -> Vec<&str> {
    let lines: Vec<&str> = log.lines().collect();
    let start = lines
        .iter()
        .position(|line| line.starts_with("Detalle del error de CanopyDB: "))
        .unwrap_or_else(|| panic!("a detail line: {log}"));
    lines[start..].to_vec()
}

fn assert_engine_failure(fault: Fault, message: impl Fn(&Scenario, &std::path::Path) -> String) {
    let scenario = Scenario::new(&IDS);
    let config = scenario.config("out");
    generate_ok(&config);
    let before = snapshot(&config.output);
    let failing = rerun(&config, "failing", true);

    let _armed = arm(fault);
    let (result, log) = generate(&failing);

    assert_eq!(exit_status_of(&result), 1, "{fault:?}: {log}");
    let tail = tail_from_detail(&log);
    assert_eq!(tail.len(), 3, "{fault:?}: {log}");
    assert!(!tail[0].contains('\n'));
    assert!(
        tail[0].len() > "Detalle del error de CanopyDB: ".len(),
        "the detail names the error: {log}"
    );
    assert_eq!(tail[1], message(&scenario, &config.output), "{fault:?}");
    assert_eq!(
        tail[2],
        "Generación cancelada. No se ha modificado el conjunto existente."
    );
    assert_eq!(snapshot(&config.output), before, "{fault:?}");
    assert_eq!(read_canopy(&dataset_dir(&config.output)).len(), 6);
}

#[test]
fn a_full_disk_in_the_engine_is_insufficient_space_in_the_output_folder() {
    assert_engine_failure(Fault::DiskFull, |_, output| {
        format!("Espacio insuficiente en {}.", output.display())
    });
}

#[test]
fn a_write_failure_in_the_engine_is_an_unwritable_output_folder() {
    assert_engine_failure(Fault::WriteFailure, |_, output| {
        format!(
            "No se puede escribir en la carpeta de salida {}.",
            output.display()
        )
    });
}

#[test]
fn an_internal_failure_of_the_engine_is_an_unwritable_output_folder() {
    assert_engine_failure(Fault::Internal, |_, output| {
        format!(
            "No se puede escribir en la carpeta de salida {}.",
            output.display()
        )
    });
}

#[test]
fn a_panic_of_the_engine_is_an_unwritable_output_folder_with_exit_code_1() {
    assert_engine_failure(Fault::Panic, |_, output| {
        format!(
            "No se puede escribir en la carpeta de salida {}.",
            output.display()
        )
    });
}

#[test]
fn a_multi_line_engine_error_is_one_detail_line() {
    assert_engine_failure(Fault::MultiLine, |_, output| {
        format!(
            "No se puede escribir en la carpeta de salida {}.",
            output.display()
        )
    });
}
