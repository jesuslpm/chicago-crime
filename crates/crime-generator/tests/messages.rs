//! Number formatting and wording of user-facing messages.

use std::io;
use std::path::Path;
use std::time::Duration;

use std::path::PathBuf;

use crime_dataset::RecordCounts;
use crime_generator::Abort;
use crime_generator::memory::MIB;
use crime_generator::messages::{
    dataset_published, duplicate_id, empty_csv, format_duration, generation_cancelled,
    generation_completed, index_sort_completed, indexes_generated, leftover_kept,
    leftovers_discarded, log_location, mebibytes, memory_limit_exceeded, operator_cancelled,
    peak_memory, previous_version_kept, published_file, reading_completed, run_duration,
    stale_sort_dir_kept, stale_sort_dirs_discarded, thousands,
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
fn publication_messages_match_the_prd() {
    assert_eq!(
        generation_cancelled(),
        "Generación cancelada. No se ha modificado el conjunto existente."
    );
    assert_eq!(
        leftovers_discarded(),
        "Se han descartado restos de una generación interrumpida anterior."
    );
    assert_eq!(
        empty_csv(),
        "Aviso: el CSV no contiene registros; el conjunto generado está vacío."
    );
}

#[test]
fn stale_sort_folder_messages_say_what_was_discarded_or_kept() {
    assert_eq!(
        stale_sort_dirs_discarded(),
        "Se han descartado carpetas temporales de una generación interrumpida anterior."
    );
    assert_eq!(
        stale_sort_dir_kept(Path::new("D:/scratch/crime-generator-sort-48804-a1B2c3")),
        "Aviso: no se ha podido eliminar la carpeta temporal \
         D:/scratch/crime-generator-sort-48804-a1B2c3; se reintentará en la próxima generación."
    );
}

#[test]
fn an_operator_cancellation_says_so_and_that_nothing_changed() {
    assert_eq!(
        operator_cancelled(),
        "Generación cancelada por el operador. No se ha modificado el conjunto existente."
    );
    assert_eq!(Abort::Cancelled.to_string(), operator_cancelled());
}

#[test]
fn the_memory_warning_matches_the_prd_and_rounds_the_peak_up() {
    assert_eq!(
        memory_limit_exceeded(580 * MIB, 512),
        "Aviso: el pico de memoria privada comprometida (580 MiB) supera el límite de 512 MiB."
    );
    // Rounded up: a peak over the limit never prints as the limit itself.
    assert_eq!(
        memory_limit_exceeded(512 * MIB + 1, 512),
        "Aviso: el pico de memoria privada comprometida (513 MiB) supera el límite de 512 MiB."
    );
    assert_eq!(
        memory_limit_exceeded(2_048 * MIB, 1),
        "Aviso: el pico de memoria privada comprometida (2.048 MiB) supera el límite de 1 MiB."
    );
}

#[test]
fn sizes_in_mib_have_one_decimal_rounded_up() {
    let cases = [
        (0, "0,0 MiB"),
        (1, "0,1 MiB"),
        (MIB, "1,0 MiB"),
        (MIB + 1, "1,1 MiB"),
        (3 * MIB / 2, "1,5 MiB"),
        (1_024 * MIB, "1.024,0 MiB"),
        (907_345_123, "865,4 MiB"),
    ];
    for (bytes, expected) in cases {
        assert_eq!(mebibytes(bytes), expected, "{bytes} bytes");
    }
}

#[test]
fn durations_are_written_in_hours_minutes_and_seconds_with_one_decimal() {
    let cases = [
        (0, "0,0 s"),
        (1_234, "1,2 s"),
        (59_940, "59,9 s"),
        (59_960, "1 min 0,0 s"),
        (63_500, "1 min 3,5 s"),
        (3_723_400, "1 h 2 min 3,4 s"),
    ];
    for (millis, expected) in cases {
        assert_eq!(
            format_duration(Duration::from_millis(millis)),
            expected,
            "{millis} ms"
        );
    }
    assert_eq!(
        run_duration(Duration::from_millis(63_500)),
        "Duración: 1 min 3,5 s."
    );
}

#[test]
fn the_peak_memory_line_shows_the_limit_and_whether_it_was_exceeded() {
    assert_eq!(
        peak_memory(Some(100 * MIB), 512),
        "Pico de memoria privada comprometida: 100,0 MiB (límite: 512 MiB)."
    );
    assert_eq!(
        peak_memory(Some(512 * MIB), 512),
        "Pico de memoria privada comprometida: 512,0 MiB (límite: 512 MiB).",
        "reaching the limit is not exceeding it"
    );
    assert_eq!(
        peak_memory(Some(512 * MIB + 1), 512),
        "Pico de memoria privada comprometida: 512,1 MiB (supera el límite de 512 MiB)."
    );
    assert_eq!(
        peak_memory(None, 512),
        "Pico de memoria privada comprometida: no disponible en este sistema."
    );
}

#[test]
fn the_summary_names_the_dataset_folder_every_file_with_its_size_and_the_log() {
    assert_eq!(
        dataset_published(Path::new(
            "D:/datasets/chicago/dataset-20261012-101500-4242"
        )),
        "Conjunto de datos publicado en D:/datasets/chicago/dataset-20261012-101500-4242."
    );
    assert_eq!(
        published_file("primary.sst", 907_345_123),
        "Archivo primary.sst: 907.345.123 bytes (865,4 MiB)."
    );
    assert_eq!(
        published_file("manifest.json", 1_234),
        "Archivo manifest.json: 1.234 bytes (0,1 MiB)."
    );
    assert_eq!(
        log_location(Path::new("D:/datasets/chicago-20261012-101500.log")),
        "Log de la ejecución: D:/datasets/chicago-20261012-101500.log."
    );
}

#[test]
fn output_folder_conflicts_name_the_folder() {
    let dir = PathBuf::from("D:/datasets/chicago");

    assert_eq!(
        Abort::OutputLocked { dir: dir.clone() }.to_string(),
        "Otra generación está usando la carpeta de salida D:/datasets/chicago."
    );
    assert_eq!(
        Abort::ForeignContent {
            dir,
            entry: "notas.txt".to_owned()
        }
        .to_string(),
        "La carpeta de salida D:/datasets/chicago contiene \"notas.txt\", que no forma parte de \
         un conjunto de datos. Usa una carpeta vacía o una que solo contenga un conjunto generado."
    );
    assert_eq!(
        previous_version_kept(Path::new("D:/datasets/chicago/dataset-1")),
        "Aviso: no se ha podido eliminar la versión anterior D:/datasets/chicago/dataset-1; se \
         descartará en la próxima generación."
    );
    assert_eq!(
        leftover_kept(Path::new("D:/datasets/chicago/dataset-20200101-000000-999")),
        "Aviso: no se ha podido eliminar el resto D:/datasets/chicago/dataset-20200101-000000-999; \
         se reintentará en la próxima generación."
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
