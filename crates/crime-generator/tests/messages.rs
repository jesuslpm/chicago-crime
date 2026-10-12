//! Number formatting and wording of user-facing messages.

use std::io;
use std::path::Path;

use crime_dataset::RecordCounts;
use crime_generator::Abort;
use crime_generator::messages::{
    duplicate_id, generation_completed, index_sort_completed, indexes_generated, reading_completed,
    thousands,
};

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
fn index_summary_lists_every_index_with_its_entries() {
    let counts = [
        ("date", 8_656_904),
        ("primary_type", 8_656_904),
        ("district", 8_656_904),
        ("community_area", 8_656_904),
        ("case_number", 8_656_904),
    ];

    assert_eq!(
        indexes_generated(&counts),
        "Índices generados: date 8.656.904, primary_type 8.656.904, district 8.656.904, \
         community_area 8.656.904, case_number 8.656.904."
    );
}

#[test]
fn index_sort_reports_its_segments() {
    assert_eq!(
        index_sort_completed(44),
        "Ordenación de los índices completada. Segmentos temporales en disco: 44."
    );
}

#[test]
fn index_build_defects_are_internal_errors() {
    assert_eq!(
        Abort::UndecodableRecord { id: 14_348_063 }.to_string(),
        "Error interno: el registro con ID 14348063 de la tabla principal no se puede \
         decodificar para generar los índices."
    );
    assert_eq!(
        Abort::IndexCountMismatch {
            index: "date",
            entries: 8_656_903,
            valid: 8_656_904
        }
        .to_string(),
        "Error interno: el índice date tiene 8.656.903 entradas, pero la tabla principal tiene \
         8.656.904 registros válidos."
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
