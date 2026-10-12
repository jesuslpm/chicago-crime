//! Command line options and the configuration they resolve to.

use std::path::PathBuf;

use crime_dataset::Compression;
use crime_generator::cli::{Config, Parsed, default_log_path, local_timestamp, parse};

fn config_from(args: &[&str]) -> Config {
    let argv = std::iter::once("crime-generator").chain(args.iter().copied());
    match parse(argv) {
        Parsed::Run(args) => Config::from_args(args, "20261012-153000"),
        other => panic!("the arguments should be accepted: {other:?}"),
    }
}

#[test]
fn defaults_use_the_system_temp_dir_snappy_and_a_log_next_to_the_output() {
    let dir = tempfile::tempdir().expect("a temporary directory can be created");
    let output = dir.path().join("dataset");
    let output_arg = output.to_str().expect("temp paths are UTF-8");

    let config = config_from(&["--input", "crimes.csv", "--output", output_arg]);

    assert_eq!(config.input, PathBuf::from("crimes.csv"));
    assert_eq!(config.output, output);
    assert_eq!(config.temp_dir, std::env::temp_dir());
    assert_eq!(config.log, dir.path().join("dataset-20261012-153000.log"));
    assert!(!config.overwrite);
    assert_eq!(config.compression, Compression::Snappy);
    assert_eq!(config.segment_size.get(), 250_000);
    assert_eq!(config.index_segment_size.get(), 1_000_000);
}

#[test]
fn hidden_segment_size_options_are_honoured() {
    let config = config_from(&[
        "--input",
        "in.csv",
        "--output",
        "out",
        "--segment-size",
        "2",
        "--index-segment-size",
        "3",
    ]);

    assert_eq!(config.segment_size.get(), 2);
    assert_eq!(config.index_segment_size.get(), 3);
}

#[test]
fn zero_segment_sizes_are_refused() {
    for option in ["--segment-size", "--index-segment-size"] {
        let parsed = parse([
            "crime-generator",
            "--input",
            "in.csv",
            "--output",
            "out",
            option,
            "0",
        ]);

        assert!(
            matches!(parsed, Parsed::Invalid(_)),
            "{option}: a segment holds at least one entry: {parsed:?}"
        );
    }
}

#[test]
fn every_option_is_honoured() {
    let config = config_from(&[
        "--input",
        "in.csv",
        "--output",
        "out",
        "--temp-dir",
        "scratch",
        "--log",
        "run.log",
        "--overwrite",
        "--no-compression",
    ]);

    assert_eq!(config.temp_dir, PathBuf::from("scratch"));
    assert_eq!(config.log, PathBuf::from("run.log"));
    assert!(config.overwrite);
    assert_eq!(config.compression, Compression::None);
}

#[test]
fn default_log_path_ignores_a_trailing_separator() {
    let dir = tempfile::tempdir().expect("a temporary directory can be created");
    let output = format!("{}/dataset/", dir.path().display());

    let log = default_log_path(output.as_ref(), "20261012-153000");

    assert_eq!(log, dir.path().join("dataset-20261012-153000.log"));
}

#[test]
fn local_timestamp_has_the_file_name_shape() {
    let timestamp = local_timestamp();

    assert_eq!(timestamp.len(), 15, "{timestamp}");
    assert!(timestamp.char_indices().all(|(index, character)| {
        if index == 8 {
            character == '-'
        } else {
            character.is_ascii_digit()
        }
    }));
}

#[test]
fn missing_required_options_are_reported_in_spanish() {
    let parsed = parse(["crime-generator", "--input", "in.csv"]);

    let Parsed::Invalid(message) = parsed else {
        panic!("--output is required: {parsed:?}");
    };
    assert!(
        message.starts_with("Faltan opciones obligatorias: --output"),
        "{message}"
    );
}

#[test]
fn unknown_option_is_reported_in_spanish() {
    let parsed = parse(["crime-generator", "--input", "a", "--output", "b", "--fast"]);

    let Parsed::Invalid(message) = parsed else {
        panic!("--fast is not an option: {parsed:?}");
    };
    assert!(
        message.starts_with("Opción desconocida: --fast"),
        "{message}"
    );
}

#[test]
fn help_is_in_spanish() {
    let Parsed::Help(text) = parse(["crime-generator", "--help"]) else {
        panic!("--help shows the help");
    };

    assert!(text.contains("Uso:"), "{text}");
    assert!(text.contains("--input <ARCHIVO>"), "{text}");
    assert!(text.contains("--no-compression"), "{text}");
    assert!(!text.contains("Usage:"), "{text}");
    assert!(
        !text.contains("segment-size"),
        "tuning options stay hidden: {text}"
    );
}
