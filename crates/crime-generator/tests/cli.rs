//! Command line options and the configuration they resolve to.

use std::path::PathBuf;

use crime_dataset::Compression;
use crime_generator::cli::{Config, Parsed, default_log_path, local_timestamp, parse};
use crime_generator::exit_code;

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
    assert_eq!(config.memory_limit_mib.get(), 512);
}

#[test]
fn the_hidden_memory_limit_option_is_honoured_and_refuses_zero() {
    let config = config_from(&[
        "--input",
        "in.csv",
        "--output",
        "out",
        "--memory-limit-mib",
        "1",
    ]);
    assert_eq!(config.memory_limit_mib.get(), 1);

    let parsed = parse([
        "crime-generator",
        "--input",
        "in.csv",
        "--output",
        "out",
        "--memory-limit-mib",
        "0",
    ]);
    assert!(matches!(parsed, Parsed::Invalid(_)), "{parsed:?}");
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
    assert!(
        !text.contains("memory-limit"),
        "the test option stays hidden: {text}"
    );
}

#[test]
fn help_lists_every_exit_code() {
    let Parsed::Help(text) = parse(["crime-generator", "--help"]) else {
        panic!("--help shows the help");
    };

    let section = text
        .split_once("Códigos de salida:\n")
        .map(|(_, section)| section)
        .unwrap_or_else(|| panic!("an exit code section: {text}"));
    for code in [
        exit_code::COMPLETED,
        exit_code::ABORTED,
        exit_code::USAGE,
        exit_code::CANCELLED,
    ] {
        assert!(
            section
                .lines()
                .any(|line| line.trim_start().starts_with(&format!("{code} "))),
            "code {code} is explained: {text}"
        );
    }
    assert!(section.contains("Ctrl+C"), "{text}");
}
