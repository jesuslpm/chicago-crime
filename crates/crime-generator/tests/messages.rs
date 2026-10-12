//! Number formatting and wording of user-facing messages.

use std::io;
use std::path::Path;

use crime_dataset::RecordCounts;
use crime_generator::Abort;
use crime_generator::messages::{duplicate_id, generation_completed, reading_completed, thousands};

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

#[test]
fn duplicate_id_names_both_rows_and_the_plain_id() {
    assert_eq!(
        duplicate_id(5_678, 14_348_063, 1_234),
        "Fila 5678 omitida: el ID 14348063 ya apareció en la fila 1234."
    );
}

#[test]
fn generation_summary_matches_the_prd_example() {
    let counts = RecordCounts {
        valid: 8_656_900,
        rejected: 4,
    };

    assert_eq!(
        generation_completed(counts),
        "Generación completada: 8.656.900 registros válidos, 4 rechazados (ver log)."
    );
}

#[test]
fn a_full_disk_is_reported_as_insufficient_space_in_that_folder() {
    let dir = Path::new("D:/scratch");
    let full = io::Error::from(io::ErrorKind::StorageFull);
    let denied = io::Error::from(io::ErrorKind::PermissionDenied);

    assert_eq!(
        Abort::temp_io(dir, &full).to_string(),
        format!("Espacio insuficiente en {}.", dir.display())
    );
    assert_eq!(
        Abort::output_io(dir, &full).to_string(),
        format!("Espacio insuficiente en {}.", dir.display())
    );
    assert_eq!(
        Abort::temp_io(dir, &denied).to_string(),
        format!("No se puede usar la carpeta temporal {}.", dir.display())
    );
    assert_eq!(
        Abort::output_io(dir, &denied).to_string(),
        format!(
            "No se puede escribir en la carpeta de salida {}.",
            dir.display()
        )
    );
}
