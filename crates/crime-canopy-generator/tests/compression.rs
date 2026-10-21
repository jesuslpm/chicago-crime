//! The compression option takes effect in the base (decision KG4): a large, compressible value
//! takes less space with CanopyDB's compression than without it. The dataset's own values are
//! too small to be compressed either way, so the storage is driven directly.

use std::fs;

use crime_canopy_generator::CanopyStorage;
use crime_dataset::IndexKind;
use crime_generator::storage::{Storage, Written};

/// Size of the base written with one primary value of 256 KiB of repeated bytes.
fn base_size(compression: bool) -> u64 {
    let dir = tempfile::tempdir().expect("a temporary directory can be created");
    let version = dir.path().join("version");
    fs::create_dir(&version).expect("the version folder");
    let mut storage = CanopyStorage::new(compression, dir.path());
    storage
        .begin(&version, dir.path())
        .expect("the base starts");
    storage
        .add_primary(b"key", &vec![b'x'; 256 * 1024])
        .expect("the value is added");
    storage.finish_primary().expect("the primary table ends");
    for kind in IndexKind::ALL {
        storage.begin_index(kind).expect("the index starts");
        storage.finish_index().expect("the index ends");
    }
    let written = storage.finish().expect("the base is closed");
    written.files().iter().map(|(_, size)| size).sum()
}

#[test]
fn compression_makes_a_large_value_take_less_space() {
    let compressed = base_size(true);
    let plain = base_size(false);

    assert!(
        compressed < plain,
        "with compression {compressed} bytes, without {plain} bytes"
    );
}
