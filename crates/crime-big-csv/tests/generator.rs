//! AC-KR5: the CanopyDB generator accepts the big CSV without rejections.

mod common;

use std::fs;

use common::{Scenario, fixture_rows};
use crime_canopy_generator::{APP, CanopyStorage};
use crime_dataset::Compression;
use crime_generator::cancel::CancelFlag;
use crime_generator::cli::Config;
use crime_generator::index::DEFAULT_INDEX_SEGMENT_SIZE;
use crime_generator::memory::DEFAULT_MEMORY_LIMIT_MIB;
use crime_generator::preflight::create_log;
use crime_generator::sort::DEFAULT_SEGMENT_SIZE;

#[test]
fn the_canopy_generator_accepts_the_big_csv_without_rejections() {
    let rows = fixture_rows();
    let scenario = Scenario::new(&rows);
    let output = scenario.run();
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let temp_dir = scenario.dir.path().join("tmp");
    fs::create_dir(&temp_dir).unwrap();
    let config = Config {
        input: scenario.output.clone(),
        output: scenario.dir.path().join("dataset"),
        temp_dir,
        log: scenario.dir.path().join("dataset.log"),
        overwrite: false,
        compression: Compression::Snappy,
        segment_size: DEFAULT_SEGMENT_SIZE,
        index_segment_size: DEFAULT_INDEX_SEGMENT_SIZE,
        memory_limit_mib: DEFAULT_MEMORY_LIMIT_MIB,
    };
    let mut log = create_log(&config.log, false).expect("the log can be created");
    let storage = CanopyStorage::new(true, &config.output);
    let result =
        crime_generator::generate_with(&config, &APP, storage, &mut log, &CancelFlag::new());
    drop(log);
    let content = fs::read_to_string(&config.log).unwrap();
    let built = result.unwrap_or_else(|abort| panic!("{abort}\n{content}"));

    let records = rows.len() as u64 * 3;
    assert_eq!(built.counts.valid, records, "{content}");
    assert_eq!(built.counts.rejected, 0, "{content}");
    assert!(
        content.contains(&format!("{records} registros válidos, 0 rechazados")),
        "{content}"
    );
}
