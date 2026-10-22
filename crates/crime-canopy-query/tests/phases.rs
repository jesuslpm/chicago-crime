//! The hidden phase markers the robustness scenarios use (canopy-db-robustez PRD): the CanopyDB
//! query app marks when its base is open and when each query starts, so a scenario can kill it
//! during the opening (R3) or in a given query of a batch (R4).
//!
//! The markers read their environment variable once per process, so this file holds a single
//! test that sets it before the first query.

mod common;

use std::fs;

use common::{CanopyFixture, fixture_rows, run};
use crime_canopy_query::APP;
use crime_dataset::probe::PHASE_FILE_ENV;

#[test]
fn a_single_query_and_a_batch_mark_the_opening_and_each_query() {
    // Set before the fixture: its generation marks phases too, and the markers read the
    // environment once.
    let marks = tempfile::tempdir().unwrap();
    let phases = marks.path().join("phases");
    // SAFETY: the only test of this binary, before any thread reads the environment.
    unsafe { std::env::set_var(PHASE_FILE_ENV, &phases) };
    let fixture = CanopyFixture::with_rows(&fixture_rows());
    fs::remove_file(&phases).unwrap();

    let single = run(&APP, &fixture.output(), &["--where", "ID=1", "--count"]);
    assert_eq!(single.code, 0, "{}", single.stderr);
    assert_eq!(fs::read_to_string(&phases).unwrap(), "opened\nquery\n");
    fs::remove_file(&phases).unwrap();

    let batch_file = fixture.dir.path().join("batch.txt");
    fs::write(&batch_file, "--where ID=1 --count\n--where ID=2 --count\n").unwrap();
    let batch = run(
        &APP,
        &fixture.output(),
        &["--batch", batch_file.to_str().unwrap()],
    );
    assert_eq!(batch.code, 0, "{}", batch.stderr);
    assert_eq!(
        fs::read_to_string(&phases).unwrap(),
        "opened\nquery\nquery\n"
    );
}
