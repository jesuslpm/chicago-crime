//! Command line options and the [`Config`] they resolve to. Help and errors are in Spanish.

use std::ffi::OsString;
use std::num::{NonZeroU64, NonZeroUsize};
use std::path::{Path, PathBuf};

use clap::error::{ContextKind, ContextValue, ErrorKind};
use clap::{ArgAction, Parser};
use crime_dataset::Compression;

use crate::index::DEFAULT_INDEX_SEGMENT_SIZE;
use crate::memory::DEFAULT_MEMORY_LIMIT_MIB;
use crate::messages;
use crate::sort::DEFAULT_SEGMENT_SIZE;

/// Fallback folder name for the default log when the output path has none (a drive root).
const FALLBACK_OUTPUT_NAME: &str = "dataset";

/// The exit codes of [`crate::exit_code`], explained at the end of the help (decision G3).
const EXIT_CODES_HELP: &str = "\
Códigos de salida:
  0    La generación ha terminado y el conjunto se ha publicado, aunque haya registros
       rechazados o un aviso de memoria.
  1    La generación se ha cancelado por un error; el conjunto existente no se ha modificado.
  2    Las opciones no son válidas; no se ha hecho nada.
  130  El operador ha cancelado la generación con Ctrl+C; el conjunto existente no se ha
       modificado.";

/// Command line options as typed by the operator.
#[derive(Debug, Parser)]
#[command(
    name = "crime-generator",
    about = "Genera el conjunto de datos de delitos de Chicago (SSTables y manifiesto) a partir \
             del CSV.",
    override_usage = "crime-generator --input <ARCHIVO> --output <CARPETA> [opciones]",
    // `{options}` and `{after-help}` already bring the line breaks that separate them.
    help_template = "{about}\n\nUso: {usage}\n\nOpciones:\n{options}{after-help}\n",
    after_help = EXIT_CODES_HELP,
    disable_help_flag = true
)]
pub struct Args {
    #[arg(
        long,
        value_name = "ARCHIVO",
        help = "CSV de delitos que se va a leer."
    )]
    pub input: PathBuf,

    #[arg(
        long,
        value_name = "CARPETA",
        help = "Carpeta donde se publica el conjunto de datos."
    )]
    pub output: PathBuf,

    #[arg(
        long,
        value_name = "CARPETA",
        help = "Carpeta para los archivos temporales de la ordenación. Por defecto, la carpeta \
                temporal del sistema."
    )]
    pub temp_dir: Option<PathBuf>,

    #[arg(
        long,
        value_name = "ARCHIVO",
        help = "Archivo de log de la ejecución. Por defecto, junto a la carpeta de salida, con \
                el nombre <carpeta>-<AAAAMMDD-hhmmss>.log."
    )]
    pub log: Option<PathBuf>,

    #[arg(
        long,
        help = "Reemplaza el conjunto de datos que ya exista en la carpeta de salida."
    )]
    pub overwrite: bool,

    #[arg(
        long,
        help = "Escribe las tablas sin compresión. Por defecto se usa Snappy."
    )]
    pub no_compression: bool,

    // Entries the external sort keeps in memory before writing a segment to disk. Hidden: it
    // exists for tests and memory tuning, not for operators. (A doc comment would become its
    // help text.)
    #[arg(long, value_name = "N", hide = true)]
    pub segment_size: Option<NonZeroUsize>,

    // Entries the index sort keeps in memory before writing a segment to disk. Hidden for the
    // same reason as `segment_size`.
    #[arg(long, value_name = "N", hide = true)]
    pub index_segment_size: Option<NonZeroUsize>,

    // Peak private committed memory, in MiB, above which the run logs a warning; 512 by default
    // (parent PRD, "Restricción principal"). Hidden: it exists so that tests can exceed it.
    #[arg(long, value_name = "MIB", hide = true)]
    pub memory_limit_mib: Option<NonZeroU64>,

    #[arg(short, long, action = ArgAction::Help, help = "Muestra esta ayuda.")]
    help: Option<bool>,
}

/// Result of parsing the command line.
#[derive(Debug)]
pub enum Parsed {
    Run(Args),
    /// The help text to print; the program then ends successfully.
    Help(String),
    /// A usage error, already translated.
    Invalid(String),
}

/// Parses the command line (`args` includes the program name).
pub fn parse<I, T>(args: I) -> Parsed
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let error = match Args::try_parse_from(args) {
        Ok(args) => return Parsed::Run(args),
        Err(error) => error,
    };
    match (error.kind(), error.get(ContextKind::InvalidArg)) {
        (ErrorKind::DisplayHelp, _) => Parsed::Help(error.render().to_string()),
        (ErrorKind::MissingRequiredArgument, Some(ContextValue::Strings(options))) => {
            Parsed::Invalid(messages::missing_options(options))
        }
        (ErrorKind::UnknownArgument, Some(ContextValue::String(option))) => {
            Parsed::Invalid(messages::unknown_option(option))
        }
        _ => Parsed::Invalid(messages::invalid_arguments()),
    }
}

/// Everything a run needs, with defaults resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub input: PathBuf,
    pub output: PathBuf,
    pub temp_dir: PathBuf,
    pub log: PathBuf,
    /// Whether an existing dataset in `output` may be replaced.
    pub overwrite: bool,
    pub compression: Compression,
    /// Entries held in memory per segment of the primary sort.
    pub segment_size: NonZeroUsize,
    /// Entries held in memory per segment of the index sort.
    pub index_segment_size: NonZeroUsize,
    /// Peak private committed memory, in MiB, above which the run logs a warning.
    pub memory_limit_mib: NonZeroU64,
}

impl Config {
    /// Resolves the defaults; `timestamp` names the default log (see [`default_log_path`]).
    pub fn from_args(args: Args, timestamp: &str) -> Self {
        let log = args
            .log
            .unwrap_or_else(|| default_log_path(&args.output, timestamp));
        Self {
            input: args.input,
            output: args.output,
            temp_dir: args.temp_dir.unwrap_or_else(std::env::temp_dir),
            log,
            overwrite: args.overwrite,
            compression: if args.no_compression {
                Compression::None
            } else {
                Compression::Snappy
            },
            segment_size: args.segment_size.unwrap_or(DEFAULT_SEGMENT_SIZE),
            index_segment_size: args
                .index_segment_size
                .unwrap_or(DEFAULT_INDEX_SEGMENT_SIZE),
            memory_limit_mib: args.memory_limit_mib.unwrap_or(DEFAULT_MEMORY_LIMIT_MIB),
        }
    }
}

/// Default log location (decision G1): next to the output folder, outside it, named
/// `<output folder name>-<timestamp>.log`.
pub fn default_log_path(output: &Path, timestamp: &str) -> PathBuf {
    // The absolute form gives `.` or `out/` a real folder name and a parent.
    let output = std::path::absolute(output).unwrap_or_else(|_| output.to_path_buf());
    let mut file_name = output
        .file_name()
        .map_or_else(|| OsString::from(FALLBACK_OUTPUT_NAME), ToOwned::to_owned);
    file_name.push(format!("-{timestamp}.log"));
    output.parent().unwrap_or(&output).join(file_name)
}

/// Local time as `YYYYMMDD-HHMMSS`, for file names.
pub fn local_timestamp() -> String {
    chrono::Local::now().format("%Y%m%d-%H%M%S").to_string()
}
