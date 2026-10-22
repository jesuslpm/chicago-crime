//! Command line options and the [`Options`] they resolve to. Help and errors are in Spanish.
//!
//! Everything checked here is the shape of the command line (exit code 2, decision C6): the
//! options clap knows, `CAMPO=VALOR`, `CAMPO=PREFIJO` and `Date=DESDE..HASTA`, the limit, the format and the
//! combinations of count only, format and output file. What the conditions say (fields, values,
//! dates) is checked by [`crate::Query::build`] and rejected with exit code 1.

use std::ffi::OsString;
use std::num::NonZeroU64;
use std::path::{Path, PathBuf};

use clap::error::{ContextKind, ContextValue, ErrorKind};
use clap::parser::ValueSource;
use clap::{ArgAction, ArgMatches, CommandFactory, FromArgMatches, Parser};

use crate::query::RawCondition;
use crate::{App, messages};

/// The exit codes of [`crate::exit_code`], explained at the end of the help (decision C6).
const EXIT_CODES_HELP: &str = "\
Códigos de salida:
  0    La consulta se ha ejecutado, aunque no haya resultados.
  1    La consulta se ha rechazado por el conjunto de datos o por sus condiciones, o ha
       abortado.
  2    La línea de comandos no es válida; no se ha consultado nada.
  130  El analista ha cancelado la consulta con Ctrl+C; no queda ningún archivo de salida a
       medias.

Con --batch, 0 indica que se han ejecutado todas las consultas del lote; 1, que el archivo
del lote no se puede leer, no es UTF-8, no tiene consultas o tiene una línea rechazada por sus
condiciones o por repetir un archivo de salida, o que una consulta se ha rechazado o ha
abortado; 2, que la línea de comandos o una línea del lote no tienen una forma válida; y 130,
que el analista ha cancelado el lote. Si se cierra la salida estándar, el lote para con 0.";

/// Command line options as typed by the analyst. The limit and the format are read as text so
/// that their errors get the PRD messages instead of clap's.
#[derive(Debug, Parser)]
// The name, the description, the usage and the end of the help come from the [`App`] (see
// [`command`]).
#[command(
    name = "crime-query",
    // `{options}` and `{after-help}` already bring the line breaks that separate them.
    help_template = "{about}\n\nUso: {usage}\n\nOpciones:\n{options}{after-help}\n",
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

    #[arg(
        long,
        value_name = "ARCHIVO",
        help = "Ejecuta un lote: las consultas de ARCHIVO, una por línea, sobre el conjunto de \
                --dataset, que se abre una vez; todas leen la misma versión. Cada línea lleva \
                las opciones de una consulta suelta (--where, --startswith, --range, --limit, \
                --count, --format y --output, también como --opción=valor), que con --batch no \
                se admiten en la línea de comandos. El archivo va en UTF-8, con o sin BOM, y con \
                finales de línea LF o CRLF. Se ignoran las líneas vacías y las que empiezan por #. \
                Los espacios separan las palabras; unas comillas dobles o simples agrupan un \
                texto con espacios, sin secuencias de escape. Antes de abrir el conjunto se \
                comprueban todas las líneas: si alguna está mal, o dos líneas usan el mismo \
                archivo de salida, no se ejecuta ninguna consulta y se indica la primera línea \
                mal. Cada consulta escribe sus resultados como suelta; en la salida de errores \
                van su cabecera (--- Consulta 2 de 3 (línea 4) ---), sus avisos y sus \
                estadísticas, y al final la línea \"Lote completado\" con el tiempo total. Si una \
                consulta falla o se cancela, el lote para ahí y las anteriores conservan sus \
                resultados."
    )]
    // Read as text so that an empty path gets the PRD's "cannot read" message, not clap's.
    #[arg(value_parser = clap::value_parser!(OsString))]
    batch: Option<OsString>,

    #[arg(short, long, action = ArgAction::Help, help = "Muestra esta ayuda.")]
    help: Option<bool>,
}

/// The options of a single query that go on each line of a batch, never next to `--batch`, by
/// clap id and as the analyst types them.
const QUERY_OPTIONS: [(&str, &str); 7] = [
    ("where", "--where"),
    ("startswith", "--startswith"),
    ("range", "--range"),
    ("limit", "--limit"),
    ("count", "--count"),
    ("format", "--format"),
    ("output", "--output"),
];

/// The options a batch line does not admit: they belong to the command line.
const NOT_IN_BATCH: [&str; 4] = ["--dataset", "--batch", "--help", "-h"];

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

/// A valid command line with `--batch`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchOptions {
    /// The generator's output folder, exactly as given.
    pub dataset: PathBuf,
    /// The batch file, exactly as given.
    pub file: PathBuf,
}

/// Result of parsing the command line.
#[derive(Debug)]
pub enum Parsed {
    Run(Options),
    /// A batch of queries, one per line of a file.
    Batch(BatchOptions),
    /// The help text to print; the program then ends successfully.
    Help(String),
    /// A command-line error, already translated.
    Invalid(String),
}

/// Parses the command line of the original query app (`args` includes the program name).
pub fn parse<I, T>(args: I) -> Parsed
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    parse_for(&crate::APP, args)
}

/// Parses the command line of the query app `app` (`args` includes the program name); its help
/// carries the name and texts of `app`.
///
/// The errors that exist without `--batch` come first, with their messages; then, with
/// `--batch`, the first query option typed next to it.
pub fn parse_for<I, T>(app: &App, args: I) -> Parsed
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let (args, matches) = match matched(app, args) {
        Ok(parsed) => parsed,
        Err(error) => return translate(&error),
    };
    let batch = args.batch.clone();
    let options = match validate(args, &matches) {
        Ok(options) => options,
        Err(message) => return Parsed::Invalid(message),
    };
    let Some(file) = batch else {
        return Parsed::Run(options);
    };
    match first_query_option(&matches) {
        Some(option) => Parsed::Invalid(messages::option_with_batch(option)),
        None => Parsed::Batch(BatchOptions {
            dataset: options.dataset,
            file: PathBuf::from(file),
        }),
    }
}

/// Parses the words of one batch line into the options of its query on `dataset`, with the
/// rules and messages of the same query typed alone. `Err` is the message, without the line
/// number; every error here is one of form (exit code 2).
pub fn parse_line(words: &[String], dataset: &Path) -> Result<Options, String> {
    if let Some(option) = words.iter().find_map(|word| not_in_batch(word)) {
        return Err(messages::option_not_in_batch(option));
    }
    let args = [
        OsString::from("crime-query"),
        "--dataset".into(),
        dataset.into(),
    ]
    .into_iter()
    .chain(words.iter().map(OsString::from));
    let (args, matches) = match matched(&crate::APP, args) {
        Ok(parsed) => parsed,
        Err(error) => {
            return Err(match translate(&error) {
                Parsed::Invalid(message) => message,
                // `--help` and `-h` were refused above and `--dataset` is always there.
                _ => messages::invalid_arguments(),
            });
        }
    };
    validate(args, &matches)
}

/// The option of [`NOT_IN_BATCH`] that `word` is, alone or as `--option=value`.
fn not_in_batch(word: &str) -> Option<&'static str> {
    let name = word.split_once('=').map_or(word, |(name, _)| name);
    NOT_IN_BATCH.into_iter().find(|&option| option == name)
}

/// The query option typed first, as the analyst types it.
fn first_query_option(matches: &ArgMatches) -> Option<&'static str> {
    // `--count` has a default value when it is not typed; only what was typed counts.
    QUERY_OPTIONS
        .into_iter()
        .filter(|&(id, _)| matches.value_source(id) == Some(ValueSource::CommandLine))
        .filter_map(|(id, option)| Some((matches.indices_of(id)?.min()?, option)))
        .min_by_key(|&(position, _)| position)
        .map(|(_, option)| option)
}

fn matched<I, T>(app: &App, args: I) -> Result<(Args, ArgMatches), clap::Error>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    command(app)
        .try_get_matches_from(args)
        .and_then(|matches| Ok((Args::from_arg_matches(&matches)?, matches)))
}

/// The command line of `app`: its name and description, its usage lines, and the exit codes with
/// its note at the end of the help.
fn command(app: &App) -> clap::Command {
    let usage = format!(
        "{name} --dataset <CARPETA> [opciones]\n     {name} --dataset <CARPETA> --batch <ARCHIVO>",
        name = app.name
    );
    let after_help = if app.help_note.is_empty() {
        EXIT_CODES_HELP.to_owned()
    } else {
        format!("{EXIT_CODES_HELP}\n\n{}", app.help_note)
    };
    Args::command()
        .name(app.name)
        .about(app.about)
        .override_usage(usage)
        .after_help(after_help)
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
