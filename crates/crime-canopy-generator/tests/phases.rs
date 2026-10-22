//! The hidden phase markers the robustness scenarios use (canopy-db-robustez PRD): a generation
//! that replaces a dataset marks each phase it enters, in order.
//!
//! The markers read their environment variable once per process, so this file holds a single
//! test that sets it before the first generation.

mod common;

use std::fs;

use common::{Scenario, generate_ok, rerun};
use crime_dataset::probe::PHASE_FILE_ENV;

#[test]
fn a_generation_marks_its_phases_in_order() {
    let scenario = Scenario::new(&[1, 2, 3]);
    let phases = scenario.dir.path().join("phases");
    // SAFETY: the only test of this binary, before any thread reads the environment.
    unsafe { std::env::set_var(PHASE_FILE_ENV, &phases) };
    let first = scenario.config("out");
    generate_ok(&first);
    fs::remove_file(&phases).unwrap();

    generate_ok(&rerun(&first, "second", true));

    assert_eq!(
        fs::read_to_string(&phases).unwrap(),
        "read\nprimary-load\nindex-write\ndump\nmanifest\ncurrent-tmp\nswitched\n"
    );
}
