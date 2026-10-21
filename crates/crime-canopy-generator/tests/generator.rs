//! The CanopyDB generator end to end (child PRD `canopy-db-generador`): the published folder and
//! manifest (AC-KG2), the same keys and values as the LevelDB tables (K2, AC-M3), the refusal of
//! the original query app (AC-KG3), a dataset of the other engine (AC-KG5), leftovers (AC-KG6),
//! the compression option (AC-KG8), the shared output lock (AC-KG14) and the help (AC-KG12).

mod common;

use std::fs;
use std::path::Path;

use common::{
    EndedProcess, Scenario, TABLES, dataset_dir, files_under, generate, generate_ok,
    generate_original, names, read_canopy, read_sst, rerun, snapshot,
};
use crime_canopy_generator::APP;
use crime_dataset::Compression;
use crime_generator::Abort;
use crime_generator::cli::{Config, Parsed, parse_for};
use crime_generator::publish::OutputLock;

const IDS: [i64; 9] = [30, 4, 17, 8, 1, 25, 12, 9, 2];

/// The top-level fields of the CanopyDB manifest, in order (child PRD, "Manifiesto").
const MANIFEST_FIELDS: [&str; 8] = [
    "schema_version",
    "generated_at",
    "engine",
    "source",
    "records",
    "compression",
    "tables",
    "files",
];

fn manifest_text(output: &Path) -> String {
    fs::read_to_string(dataset_dir(output).join("manifest.json")).expect("the manifest exists")
}

fn manifest(output: &Path) -> serde_json::Value {
    serde_json::from_str(&manifest_text(output)).expect("the manifest is JSON")
}

#[test]
fn the_version_folder_holds_only_the_manifest_and_the_base() {
    let scenario = Scenario::new(&IDS);
    let config = scenario.config("out");

    generate_ok(&config);

    let version = dataset_dir(&config.output);
    let output_names = names(&config.output);
    assert_eq!(output_names.len(), 2, "{output_names:?}");
    assert!(
        output_names.contains(&"CURRENT".to_owned()),
        "{output_names:?}"
    );
    assert_eq!(names(&version), ["canopydb", "manifest.json"]);
}

#[test]
fn the_manifest_has_the_canopydb_fields_in_order() {
    let scenario = Scenario::new(&IDS);
    let config = scenario.config("out");

    generate_ok(&config);

    let text = manifest_text(&config.output);
    assert!(text.ends_with("}\n"), "one line feed at the end: {text:?}");
    let positions: Vec<usize> = MANIFEST_FIELDS
        .iter()
        .map(|field| {
            text.find(&format!("\n  \"{field}\":"))
                .unwrap_or_else(|| panic!("top-level field {field}: {text}"))
        })
        .collect();
    assert!(positions.is_sorted(), "{text}");
    let manifest = manifest(&config.output);
    let fields = manifest.as_object().expect("an object").len();
    assert_eq!(fields, MANIFEST_FIELDS.len(), "{text}");
    assert_eq!(manifest["schema_version"], "canopydb-2");
    assert_eq!(
        manifest["engine"],
        serde_json::json!({"name": "canopydb", "version": "0.2.5", "fork_commit": null})
    );
    assert_eq!(manifest["compression"], "lz4");
    assert_eq!(
        manifest["records"],
        serde_json::json!({"valid": 9, "rejected": 0})
    );
    assert_eq!(manifest["source"]["file_name"], "crimes.csv");
    let tables: Vec<(String, u64)> = manifest["tables"]
        .as_array()
        .expect("a list")
        .iter()
        .map(|table| {
            (
                table["name"].as_str().expect("a name").to_owned(),
                table["entries"].as_u64().expect("a count"),
            )
        })
        .collect();
    let expected: Vec<(String, u64)> = TABLES.iter().map(|name| ((*name).to_owned(), 9)).collect();
    assert_eq!(tables, expected);
    assert!(text.contains("\n    {\n      \"name\": \"primary\",\n      \"entries\": 9\n    },"));
}

#[test]
fn the_manifest_files_and_the_summary_list_every_file_of_the_base() {
    let scenario = Scenario::new(&IDS);
    let config = scenario.config("out");

    let (_, log) = generate_ok(&config);

    let version = dataset_dir(&config.output);
    let on_disk = files_under(&version, &version.join("canopydb"));
    assert!(
        on_disk.iter().any(|(path, _)| path == "canopydb/LOCK"),
        "{on_disk:?}"
    );
    assert!(
        on_disk.iter().any(|(path, _)| path.ends_with("/DATA")),
        "{on_disk:?}"
    );
    let listed: Vec<(String, u64)> = manifest(&config.output)["files"]
        .as_array()
        .expect("a list")
        .iter()
        .map(|file| {
            (
                file["path"].as_str().expect("a path").to_owned(),
                file["size_bytes"].as_u64().expect("a size"),
            )
        })
        .collect();
    assert_eq!(listed, on_disk);

    let summary_files: Vec<&str> = log
        .lines()
        .filter(|line| line.starts_with("Archivo "))
        .collect();
    let mut expected: Vec<String> = on_disk
        .iter()
        .map(|(path, _)| format!("Archivo {path}: "))
        .collect();
    expected.push("Archivo manifest.json: ".to_owned());
    assert_eq!(summary_files.len(), expected.len(), "{log}");
    for (line, start) in summary_files.iter().zip(&expected) {
        assert!(line.starts_with(start.as_str()), "{line} vs {start}\n{log}");
    }
}

#[test]
fn the_base_holds_the_same_keys_and_values_as_the_leveldb_tables() {
    let scenario = Scenario::new(&IDS);
    let canopy = scenario.config("canopy");
    let original = scenario.config("original");

    generate_ok(&canopy);
    let (result, log) = generate_original(&original);
    result.unwrap_or_else(|abort| panic!("{abort}\n{log}"));

    let from_canopy = read_canopy(&dataset_dir(&canopy.output));
    assert_eq!(from_canopy, read_sst(&dataset_dir(&original.output)));
    assert!(from_canopy.iter().all(|(_, entries)| entries.len() == 9));
}

#[test]
fn two_generations_and_both_compressions_hold_the_same_content() {
    let scenario = Scenario::new(&IDS);
    let first = scenario.config("first");
    let second = scenario.config("second");
    let plain = Config {
        compression: Compression::None,
        ..scenario.config("plain")
    };

    for config in [&first, &second, &plain] {
        generate_ok(config);
    }

    let content = read_canopy(&dataset_dir(&first.output));
    assert_eq!(read_canopy(&dataset_dir(&second.output)), content);
    assert_eq!(read_canopy(&dataset_dir(&plain.output)), content);
}

#[test]
fn the_no_compression_option_is_recorded_in_the_manifest() {
    let scenario = Scenario::new(&IDS);
    let plain = Config {
        compression: Compression::None,
        ..scenario.config("plain")
    };

    generate_ok(&plain);

    assert_eq!(manifest(&plain.output)["compression"], "none");
}

#[test]
fn a_csv_with_only_the_header_publishes_six_empty_tables() {
    let scenario = Scenario::new(&[]);
    let config = scenario.config("out");

    let (_, log) = generate_ok(&config);

    assert!(
        log.contains("Aviso: el CSV no contiene registros; el conjunto generado está vacío."),
        "{log}"
    );
    let manifest = manifest(&config.output);
    assert!(
        manifest["tables"]
            .as_array()
            .expect("a list")
            .iter()
            .all(|table| table["entries"] == 0),
        "{manifest}"
    );
    assert!(!manifest["files"].as_array().expect("a list").is_empty());
    let tables = read_canopy(&dataset_dir(&config.output));
    assert_eq!(tables.len(), 6);
    assert!(tables.iter().all(|(_, entries)| entries.is_empty()));
}

#[test]
fn reading_the_base_again_adds_no_file_and_keeps_the_manifest() {
    let scenario = Scenario::new(&IDS);
    let config = scenario.config("out");
    generate_ok(&config);
    let version = dataset_dir(&config.output);
    let names_before: Vec<String> = files_under(&version, &version)
        .into_iter()
        .map(|(path, _)| path)
        .collect();
    let manifest_before = manifest_text(&config.output);

    let first = read_canopy(&version);
    let second = read_canopy(&version);

    let names_after: Vec<String> = files_under(&version, &version)
        .into_iter()
        .map(|(path, _)| path)
        .collect();
    assert_eq!(names_after, names_before);
    assert_eq!(manifest_text(&config.output), manifest_before);
    assert_eq!(first, second);
}

#[test]
fn the_original_query_app_refuses_a_canopydb_dataset() {
    let scenario = Scenario::new(&IDS);
    let config = scenario.config("out");
    generate_ok(&config);
    let dataset = config.output.to_string_lossy().into_owned();

    let crime_query::cli::Parsed::Run(options) = crime_query::cli::parse([
        "crime-query",
        "--dataset",
        dataset.as_str(),
        "--where",
        "ID=4",
        "--count",
    ]) else {
        panic!("the query is valid");
    };
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let code = crime_query::run(&options, &|| false, &mut stdout, &mut stderr);

    let stderr = String::from_utf8(stderr).expect("UTF-8");
    assert_eq!(code, 1, "{stderr}");
    assert_eq!(
        stderr.trim_end(),
        format!(
            "No se encontró un conjunto de datos válido en {}.",
            config.output.display()
        )
    );
    assert!(stdout.is_empty());
}

#[test]
fn a_leveldb_dataset_is_refused_without_overwrite_and_replaced_with_it() {
    let scenario = Scenario::new(&IDS);
    let config = scenario.config("out");
    let (result, log) = generate_original(&config);
    result.unwrap_or_else(|abort| panic!("{abort}\n{log}"));
    let sst_version = dataset_dir(&config.output);
    let before = snapshot(&config.output);

    let (refused, log) = generate(&rerun(&config, "refused", false));

    assert_eq!(refused.err(), Some(Abort::DatasetExists), "{log}");
    assert!(
        log.contains(
            "La carpeta de salida ya contiene un conjunto de datos. Usa la opción de sobrescritura \
             para reemplazarlo."
        ),
        "{log}"
    );
    assert_eq!(snapshot(&config.output), before);

    generate_ok(&rerun(&config, "replaced", true));

    assert!(!sst_version.exists());
    assert_eq!(manifest(&config.output)["schema_version"], "canopydb-2");
}

#[test]
fn the_original_generator_treats_a_canopydb_dataset_the_same_way() {
    let scenario = Scenario::new(&IDS);
    let config = scenario.config("out");
    generate_ok(&config);
    let canopy_version = dataset_dir(&config.output);
    let before = snapshot(&config.output);

    let (refused, log) = generate_original(&rerun(&config, "refused", false));

    assert_eq!(refused.err(), Some(Abort::DatasetExists), "{log}");
    assert_eq!(snapshot(&config.output), before);

    let (replaced, log) = generate_original(&rerun(&config, "replaced", true));

    replaced.unwrap_or_else(|abort| panic!("{abort}\n{log}"));
    assert!(!canopy_version.exists());
    assert_eq!(manifest(&config.output)["schema_version"], 2);
}

#[test]
fn orphan_version_folders_of_both_engines_are_discarded_with_one_message() {
    let scenario = Scenario::new(&IDS);
    let config = scenario.config("out");
    generate_ok(&config);
    let canopy_orphan = config.output.join("dataset-20260101-000000-1");
    let sst_orphan = config.output.join("dataset-20260101-000000-2");
    fs::create_dir_all(canopy_orphan.join("canopydb")).expect("an orphan");
    fs::write(canopy_orphan.join("canopydb").join("LOCK"), b"").expect("a file");
    fs::create_dir(&sst_orphan).expect("an orphan");
    fs::write(sst_orphan.join("primary.sst"), b"half").expect("a file");

    let (_, log) = generate_ok(&rerun(&config, "again", true));

    assert!(!canopy_orphan.exists());
    assert!(!sst_orphan.exists());
    let message = "Se han descartado restos de una generación interrumpida anterior.";
    assert_eq!(log.matches(message).count(), 1, "{log}");
}

#[test]
fn only_the_stale_sort_folders_of_this_generator_are_discarded() {
    let scenario = Scenario::new(&IDS);
    let ended = EndedProcess::new();
    let stale = scenario
        .temp_dir
        .join(format!("crime-canopy-generator-sort-{}-abc123", ended.id()));
    let running = scenario.temp_dir.join(format!(
        "crime-canopy-generator-sort-{}-abc123",
        std::process::id()
    ));
    let original = scenario
        .temp_dir
        .join(format!("crime-generator-sort-{}-abc123", ended.id()));
    for dir in [&stale, &running, &original] {
        fs::create_dir(dir).expect("a sort folder");
    }

    let (_, log) = generate_ok(&scenario.config("out"));

    assert!(!stale.exists());
    assert!(running.exists());
    assert!(original.exists());
    assert!(
        log.contains(
            "Se han descartado carpetas temporales de una generación interrumpida anterior."
        ),
        "{log}"
    );
}

#[test]
fn both_generators_share_the_lock_of_the_output_folder() {
    let scenario = Scenario::new(&IDS);
    let config = scenario.config("out");
    fs::create_dir(&config.output).expect("the output folder");
    let message = format!(
        "Otra generación está usando la carpeta de salida {}.",
        config.output.display()
    );

    let lock = OutputLock::acquire(&config.output).expect("the lock is free");
    let (canopy, canopy_log) = generate(&config);
    let (original, original_log) = generate_original(&rerun(&config, "original", false));
    drop(lock);

    assert!(
        matches!(canopy, Err(Abort::OutputLocked { .. })),
        "{canopy_log}"
    );
    assert!(canopy_log.contains(&message), "{canopy_log}");
    assert!(
        matches!(original, Err(Abort::OutputLocked { .. })),
        "{original_log}"
    );
    assert!(original_log.contains(&message), "{original_log}");
}

#[test]
fn the_help_is_the_original_one_with_the_three_changes() {
    let Parsed::Help(text) = parse_for(&APP, ["crime-canopy-generator", "--help"]) else {
        panic!("--help shows the help");
    };

    assert_eq!(text, HELP);
}

const HELP: &str = "\
Genera el conjunto de datos de delitos de Chicago (base CanopyDB y manifiesto) a partir del CSV.

Uso: crime-canopy-generator --input <ARCHIVO> --output <CARPETA> [opciones]

Opciones:
      --input <ARCHIVO>     CSV de delitos que se va a leer.
      --output <CARPETA>    Carpeta donde se publica el conjunto de datos.
      --temp-dir <CARPETA>  Carpeta para los archivos temporales de la ordenación. Por defecto, la carpeta temporal del sistema.
      --log <ARCHIVO>       Archivo de log de la ejecución. Por defecto, junto a la carpeta de salida, con el nombre <carpeta>-<AAAAMMDD-hhmmss>.log.
      --overwrite           Reemplaza el conjunto de datos que ya exista en la carpeta de salida.
      --no-compression      Escribe la base sin compresión. Por defecto, CanopyDB comprime con LZ4 los valores grandes.
  -h, --help                Muestra esta ayuda.

Códigos de salida:
  0    La generación ha terminado y el conjunto se ha publicado, aunque haya registros
       rechazados o un aviso de memoria.
  1    La generación se ha cancelado por un error; el conjunto existente no se ha modificado.
  2    Las opciones no son válidas; no se ha hecho nada.
  130  El operador ha cancelado la generación con Ctrl+C; el conjunto existente no se ha
       modificado.
";

#[test]
fn the_canopydb_version_matches_the_lock_file() {
    let lock = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.lock"))
        .expect("the lock file exists");
    let entry = lock
        .split("[[package]]")
        .find(|package| package.contains("name = \"canopydb\""))
        .expect("canopydb is locked");
    assert!(
        entry.contains(&format!(
            "version = \"{}\"",
            crime_canopy_generator::CANOPYDB_VERSION
        )),
        "{entry}"
    );
}

/// A cancellation while the base is open (AC-G7, AC-L4): the new version folder disappears
/// entirely, base included, and the previous dataset is intact.
#[test]
fn a_cancellation_while_the_base_is_open_leaves_nothing_behind() {
    use crime_generator::fail::{FailPoint, arm_after};

    let scenario = Scenario::new(&IDS);
    let config = scenario.config("out");
    generate_ok(&config);
    let before = snapshot(&config.output);

    // The merges pass their point once per entry; the switch, once per run.
    for (point, passes) in [
        (FailPoint::CancelInPrimaryMerge, 3),
        (FailPoint::CancelInIndexWrite, 3),
        (FailPoint::CancelBeforeSwitch, 0),
    ] {
        let _armed = arm_after(point, passes);
        let (result, log) = generate(&rerun(&config, &format!("cancelled-{point:?}"), true));

        assert_eq!(result.err(), Some(Abort::Cancelled), "{point:?}: {log}");
        assert_eq!(snapshot(&config.output), before, "{point:?}");
        assert_eq!(names(&scenario.temp_dir), Vec::<String>::new(), "{point:?}");
    }
}

/// A failed first generation removes the output folder it created (AC-L4).
#[test]
fn a_cancelled_first_generation_removes_the_output_folder() {
    use crime_generator::fail::{FailPoint, arm};

    let scenario = Scenario::new(&IDS);
    let config = scenario.config("out");

    let _armed = arm(FailPoint::CancelInIndexWrite);
    let (result, log) = generate(&config);

    assert_eq!(result.err(), Some(Abort::Cancelled), "{log}");
    assert!(!config.output.exists(), "{log}");
}
