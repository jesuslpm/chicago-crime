//! Command line options and the [`Options`] they resolve to. Help and errors are in Spanish.
//!
//! Everything checked here is the shape of the command line (exit code 2, decision C6): the
//! options clap knows, `CAMPO=VALOR`, `CAMPO=PREFIJO` and `Date=DESDE..HASTA`, the limit, the format and the
//! combinations of count only, format and output file. What the conditions say (fields, values,
//! dates) is checked by [`crate::Query::build`] and rejected with exit code 1.

use std::ffi::OsString;
use std::num::NonZeroU64;
use std::path::PathBuf;

use clap::error::{ContextKind, ContextValue, ErrorKind};
use clap::{ArgAction, ArgMatches, CommandFactory, FromArgMatches, Parser};

use crate::messages;
use crate::query::RawCondition;

/// The exit codes of [`crate::exit_code`], explained at the end of the help (decision C6).
const EXIT_CODES_HELP: &str = "\
Códigos de salida:
  0    La consulta se ha ejecutado, aunque no haya resultados.
  1    La consulta se ha rechazado por el conjunto de datos o por sus condiciones, o ha
       abortado.
  2    La línea de comandos no es válida; no se ha consultado nada.
  130  El analista ha cancelado la consulta con Ctrl+C; no queda ningún archivo de salida a
       medias.";

/// Command line options as typed by the analyst. The limit and the format are read as text so
/// that their errors get the PRD messages instead of clap's.
#[derive(Debug, Parser)]
#[command(
    name = "crime-query",
    about = "Consulta el conjunto de datos de delitos de Chicago publicado por el generador.",
    override_usage = "crime-query --dataset <CARPETA> [opciones]",
    // `{options}` and `{after-help}` already bring the line breaks that separate them.
    help_template = "{about}\n\nUso: {usage}\n\nOpciones:\n{options}{after-help}\n",
    after_help = EXIT_CODES_HELP,
    disable_help_flag = true
)]
struct Args {
    #[arg(
        long,
        value_name = "CARPETA",
        help = "Carpeta de salida del generador, la que contiene el conjunto de datos publicado."
    )]
    dataset: PathBuf,

    #[arg(
        long = "where",
        id = "where",
        value_name = "CAMPO=VALOR",
        help = "Condición de igualdad. CAMPO es el nombre de la columna del CSV, sin distinguir \
                mayúsculas. Repetida sobre el mismo campo da valores alternativos (O); sobre \
                campos distintos, todas se cumplen (Y). El texto se compara sin distinguir \
                mayúsculas y los códigos conservan los ceros a la izquierda. CAMPO= (nada tras \
                el =) pide \"sin valor\". Arrest y Domestic admiten true, false, sí, si o no. \
                Las fechas van como AAAA-MM-DD [hh:mm] y abarcan el día o el minuto entero."
    )]
    conditions: Vec<String>,

    #[arg(
        long = "startswith",
        id = "startswith",
        value_name = "CAMPO=PREFIJO",
        help = "Condición \"empieza por\": el campo empieza por PREFIJO, sin distinguir \
                mayúsculas; el prefijo se toma tal cual, espacios incluidos, y los códigos no \
                se completan con ceros. Solo en campos de texto. Sobre Case Number, Primary \
                Type y District es una condición indexada y puede ir sola; sobre IUCR, Location \
                Description, Beat y FBI Code es un filtro sin índice. Los prefijos y los valores \
                de --where sobre un mismo campo son alternativos (O). Un campo \"sin valor\" no \
                empieza por ningún prefijo."
    )]
    prefixes: Vec<String>,

    #[arg(
        long,
        value_name = "Date=DESDE..HASTA",
        help = "Rango de fechas inclusivo, como AAAA-MM-DD [hh:mm]. Se puede omitir uno de los \
                extremos: Date=2020-01-01.. o Date=..2020-01-31. Varios rangos se cumplen a la \
                vez (Y)."
    )]
    range: Vec<String>,

    #[arg(
        long,
        value_name = "N",
        allow_negative_numbers = true,
        help = "Devuelve como mucho N resultados."
    )]
    limit: Option<String>,

    #[arg(long, help = "Muestra solo el número de resultados.")]
    count: bool,

    #[arg(
        long,
        value_name = "FORMATO",
        help = "Formato de los resultados: table (por defecto), csv o json. Los resultados van \
                a la salida estándar; los avisos y las estadísticas, a la salida de errores."
    )]
    format: Option<String>,

    #[arg(
        long,
        value_name = "ARCHIVO",
        help = "Escribe los resultados en este archivo en lugar de la salida estándar; solo con \
                --format csv o json. Si ya existe, se sobrescribe sin preguntar. Si la consulta \
                se rechaza, falla o se cancela, el archivo no se crea ni se modifica. Mientras \
                se escribe, los resultados van a un archivo temporal junto al destino, \
                .<ARCHIVO>.XXXXXX.crime-query.tmp, que al final se renombra como ARCHIVO; si \
                el proceso se termina a la fuerza (consola cerrada, taskkill /F, corte de \
                luz), puede quedar ese archivo temporal; se puede borrar."
    )]
    output: Option<PathBuf>,

    #[arg(short, long, action = ArgAction::Help, help = "Muestra esta ayuda.")]
    help: Option<bool>,
}

/// How the records are written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Table,
    Csv,
    Json,
}

/// What the query prints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Only the number of results.
    Count,
    /// The records, in this format.
    Records(Format),
}

/// A valid command line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    /// The generator's output folder, exactly as given.
    pub dataset: PathBuf,
    /// `--where`, `--range` and `--startswith` in command-line order, split but not yet checked.
    pub conditions: Vec<RawCondition>,
    pub limit: Option<NonZeroU64>,
    pub mode: Mode,
    /// The output file, only with CSV or JSON.
    pub output: Option<PathBuf>,
}

/// Result of parsing the command line.
#[derive(Debug)]
pub enum Parsed {
    Run(Options),
    /// The help text to print; the program then ends successfully.
    Help(String),
    /// A command-line error, already translated.
    Invalid(String),
}

/// Parses the command line (`args` includes the program name).
pub fn parse<I, T>(args: I) -> Parsed
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let parsed = Args::command()
        .try_get_matches_from(args)
        .and_then(|matches| Ok((Args::from_arg_matches(&matches)?, matches)));
    let (args, matches) = match parsed {
        Ok(parsed) => parsed,
        Err(error) => return translate(&error),
    };
    match validate(args, &matches) {
        Ok(options) => Parsed::Run(options),
        Err(message) => Parsed::Invalid(message),
    }
}

fn translate(error: &clap::Error) -> Parsed {
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

fn validate(args: Args, matches: &ArgMatches) -> Result<Options, String> {
    let conditions = conditions_in_order(&args, matches)?;
    let limit = args.limit.as_deref().map(parse_limit).transpose()?;
    let format = args.format.as_deref().map(parse_format).transpose()?;
    let mode = match (args.count, format) {
        (true, Some(_)) => return Err(messages::count_with_format()),
        (true, None) => Mode::Count,
        (false, format) => Mode::Records(format.unwrap_or(Format::Table)),
    };
    if args.output.is_some() && !matches!(mode, Mode::Records(Format::Csv | Format::Json)) {
        return Err(messages::output_file_format());
    }
    Ok(Options {
        dataset: args.dataset,
        conditions,
        limit,
        mode,
        output: args.output,
    })
}

/// Every `--where`, `--range` and `--startswith`, split, in the order they were typed, so that
/// the first wrong one is the one reported.
fn conditions_in_order(args: &Args, matches: &ArgMatches) -> Result<Vec<RawCondition>, String> {
    type Split = fn(&str) -> Result<RawCondition, String>;
    let options: [(&str, &[String], Split); 3] = [
        ("where", &args.conditions, split_condition),
        ("range", &args.range, split_range),
        ("startswith", &args.prefixes, split_prefix),
    ];
    let mut typed: Vec<(usize, &str, Split)> = Vec::new();
    for (id, texts, split) in options {
        let positions = matches.indices_of(id).into_iter().flatten();
        for (position, text) in positions.zip(texts) {
            typed.push((position, text, split));
        }
    }
    typed.sort_unstable_by_key(|&(position, ..)| position);
    typed
        .into_iter()
        .map(|(_, text, split)| split(text))
        .collect()
}

/// `CAMPO=VALOR`, split at the first `=`.
fn split_condition(text: &str) -> Result<RawCondition, String> {
    let (field, value) = text
        .split_once('=')
        .ok_or_else(|| messages::malformed_condition(text))?;
    Ok(RawCondition::equal(field, value))
}

/// `CAMPO=PREFIJO`, split at the first `=`; an empty prefix is checked with the field.
fn split_prefix(text: &str) -> Result<RawCondition, String> {
    let (field, prefix) = text
        .split_once('=')
        .ok_or_else(|| messages::malformed_prefix(text))?;
    Ok(RawCondition::starts_with(field, prefix))
}

/// `CAMPO=DESDE..HASTA` with at least one end.
fn split_range(text: &str) -> Result<RawCondition, String> {
    let (field, ends) = text
        .split_once('=')
        .ok_or_else(|| messages::malformed_range(text))?;
    let (from, to) = ends
        .split_once("..")
        .ok_or_else(|| messages::malformed_range(text))?;
    if from.is_empty() && to.is_empty() {
        return Err(messages::empty_range(text));
    }
    Ok(RawCondition::range(field, from, to))
}

fn parse_limit(text: &str) -> Result<NonZeroU64, String> {
    text.parse::<u64>()
        .ok()
        .and_then(NonZeroU64::new)
        .ok_or_else(messages::invalid_limit)
}

fn parse_format(text: &str) -> Result<Format, String> {
    [
        ("table", Format::Table),
        ("csv", Format::Csv),
        ("json", Format::Json),
    ]
    .into_iter()
    .find(|(name, _)| name.eq_ignore_ascii_case(text))
    .map(|(_, format)| format)
    .ok_or_else(|| messages::invalid_format(text))
}
