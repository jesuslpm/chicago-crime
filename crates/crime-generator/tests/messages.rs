//! Number formatting in user-facing messages.

use crime_dataset::RecordCounts;
use crime_generator::messages::{reading_completed, thousands};

#[test]
fn counts_use_spanish_thousands_separators() {
    let cases = [
        (0, "0"),
        (999, "999"),
        (1_000, "1.000"),
        (632_000, "632.000"),
        (8_656_904, "8.656.904"),
        (2_406_401_982, "2.406.401.982"),
    ];
    for (value, expected) in cases {
        assert_eq!(thousands(value), expected);
    }
}

#[test]
fn reading_summary_reports_valid_and_rejected_rows() {
    let counts = RecordCounts {
        valid: 8_656_900,
        rejected: 4,
    };

    assert_eq!(
        reading_completed(counts),
        "Lectura completada: 8.656.900 registros válidos, 4 rechazados."
    );
}
