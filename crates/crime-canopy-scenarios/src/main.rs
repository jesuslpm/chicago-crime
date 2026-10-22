//! `crime-canopy-scenarios`: runs the robustness scenarios of the CanopyDB apps
//! (canopy-db-robustez PRD) and writes the rows of the scenario log.

use std::ffi::OsString;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use clap::Parser;
use crime_canopy_scenarios::context::{Binaries, Context, MIN_REPETITIONS, progress};
use crime_canopy_scenarios::findings;
use crime_canopy_scenarios::process::{CTRL_C_HOST_ARG, run_ctrl_c_host};
use crime_canopy_scenarios::report::{
    self, Associations, VariantResult, log_row, merge_repetitions, state_of,
};
use crime_canopy_scenarios::scenarios::{self, Scenario};

/// Ejecuta los escenarios de robustez de las aplicaciones CanopyDB (R1–R13) y escribe las filas
/// del registro de escenarios.
#[derive(Debug, Parser)]
#[command(name = "crime-canopy-scenarios")]
struct Cli {
    /// CSV de referencia.
    #[arg(long, value_name = "ARCHIVO", required_unless_present = "list")]
    csv: Option<PathBuf>,
    /// Carpeta de trabajo: el conjunto de referencia, sus copias y cada ejecución.
    #[arg(long, value_name = "CARPETA", required_unless_present = "list")]
    work: Option<PathBuf>,
    /// Escenarios que se ejecutan, separados por comas (por ejemplo R1,R5). Por defecto, todos.
    #[arg(long, value_name = "LISTA", value_delimiter = ',')]
    only: Vec<String>,
    /// Repeticiones de cada escenario con carreras (R8, R9, R11 y R12); como mínimo 5.
    #[arg(long, value_name = "N", default_value_t = MIN_REPETITIONS)]
    repeat: usize,
    /// Comprobaciones asociadas a un hallazgo conocido: <escenario>/<variante>/<comprobación>=<hallazgo>, separadas por comas.
    #[arg(long, value_name = "ASOCIACIONES")]
    not_met: Vec<String>,
    /// Registro de hallazgos.
    #[arg(
        long,
        value_name = "ARCHIVO",
        default_value = "docs/canopy-db/hallazgos.md"
    )]
    findings: PathBuf,
    /// Conjunto fijo de consultas (KC7).
    #[arg(
        long,
        value_name = "ARCHIVO",
        default_value = "scripts/acceptance/canopy-fixed-queries.txt"
    )]
    fixed_queries: PathBuf,
    /// Carpeta de los binarios. Por defecto, la de este programa.
    #[arg(long, value_name = "CARPETA")]
    bin_dir: Option<PathBuf>,
    /// Muestra los escenarios, sus variantes y sus comprobaciones, sin ejecutar nada.
    #[arg(long)]
    list: bool,
}

fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().collect();
    if args.get(1).is_some_and(|arg| arg == CTRL_C_HOST_ARG) {
        std::process::exit(run_ctrl_c_host(&args[2..]));
    }
    let cli = Cli::parse();
    match run(&cli) {
        Ok(code) => ExitCode::from(code),
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(1)
        }
    }
}

fn run(cli: &Cli) -> Result<u8, String> {
    let catalog = scenarios::catalog();
    if cli.list {
        for scenario in &catalog {
            for (variant, checks) in &scenario.variants {
                println!("{}  {}", scenario.variant_id(variant), checks.join(", "));
            }
        }
        return Ok(0);
    }
    let unknown: Vec<&String> = cli
        .only
        .iter()
        .filter(|id| !catalog.iter().any(|scenario| scenario.id == id.as_str()))
        .collect();
    if !unknown.is_empty() {
        return Err(format!("Escenarios desconocidos: {unknown:?}."));
    }
    // Every association is checked before anything runs (PRD, "Salida").
    let associations = Associations::parse(&cli.not_met)?;
    let finding_ids = findings::read(&cli.findings).map_err(|error| error.to_string())?;
    associations.validate(&scenarios::variant_checks(&catalog), &finding_ids)?;

    let bin_dir = match &cli.bin_dir {
        Some(dir) => dir.clone(),
        None => std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(Path::to_path_buf))
            .ok_or("No se encontró la carpeta de los binarios.")?,
    };
    let bins = Binaries::in_dir(&bin_dir);
    let big = cli.only.is_empty() || cli.only.iter().any(|id| id == "R13");
    let missing = bins.missing(big);
    if !missing.is_empty() {
        return Err(format!(
            "Faltan binarios (compila con cargo build --release --workspace): {missing:?}."
        ));
    }
    let (Some(csv), Some(work)) = (&cli.csv, &cli.work) else {
        return Err("Faltan --csv y --work.".to_owned());
    };
    let context = Context::new(bins, csv, work, &cli.fixed_queries, cli.repeat)
        .map_err(|error| error.to_string())?;

    let selected: Vec<&Scenario> = catalog
        .iter()
        .filter(|scenario| cli.only.is_empty() || cli.only.iter().any(|id| id == scenario.id))
        .collect();
    let system = if cfg!(windows) { "Windows" } else { "Linux" };
    let date = chrono::Local::now();
    let commit = git_commit();
    let mut rows = Vec::new();
    let mut states = Vec::new();
    for scenario in selected {
        let results = run_scenario(&context, scenario);
        for result in results {
            let state = state_of(&result, &associations);
            println!("{}", report::line(&result, &state));
            let _ = std::io::stdout().flush();
            if let Some(row) = log_row(
                &result,
                &state,
                system,
                &date.format("%Y-%m-%d").to_string(),
                crime_dataset::canopy::CANOPYDB_VERSION,
                &commit,
            ) {
                rows.push(row);
            }
            states.push(state);
        }
    }
    let (summary, code) = report::summary(&states);
    println!("{summary}");
    let log = work.join(format!(
        "registro-{system}-{}.md",
        date.format("%Y%m%d-%H%M%S")
    ));
    let mut text = String::from(
        "| Escenario | Sistema | Fecha | Versión de CanopyDB | Commit del proyecto | Resultado | Observado |\n|---|---|---|---|---|---|---|\n",
    );
    for row in rows {
        text.push_str(&row);
        text.push('\n');
    }
    fs::write(&log, text).map_err(|error| error.to_string())?;
    println!("Filas del registro de escenarios: {}", log.display());
    Ok(code)
}

/// Runs `scenario` in this system, its repetitions included, and returns one result per variant.
fn run_scenario(context: &Context, scenario: &Scenario) -> Vec<VariantResult> {
    if let Some(reason) = scenario.systems.not_here() {
        return scenario
            .variants
            .iter()
            .map(|(variant, _)| VariantResult::not_applicable(scenario.variant_id(variant), reason))
            .collect();
    }
    let times = if scenario.racy {
        context.repetitions
    } else {
        1
    };
    let mut repetitions = Vec::new();
    for number in 1..=times {
        progress(&format!("{} (repetición {number} de {times})", scenario.id));
        repetitions.push((scenario.run)(context));
    }
    let results = if scenario.racy {
        merge_repetitions(repetitions)
    } else {
        repetitions.into_iter().flatten().collect()
    };
    // Every declared variant has a result, even if the scenario forgot one.
    scenario
        .variants
        .iter()
        .map(|(variant, _)| {
            let id = scenario.variant_id(variant);
            results
                .iter()
                .find(|result| result.id == id)
                .cloned()
                .unwrap_or_else(|| VariantResult::not_run(id, "el escenario no dio resultado"))
        })
        .collect()
}

/// The short commit of the repository, with `+cambios` when the tree has changes.
fn git_commit() -> String {
    let git = |args: &[&str]| {
        Command::new("git")
            .args(args)
            .output()
            .ok()
            .filter(|out| out.status.success())
            .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_owned())
    };
    match git(&["rev-parse", "--short", "HEAD"]) {
        Some(commit) => {
            let dirty = git(&["status", "--porcelain", "--untracked-files=no"])
                .is_some_and(|status| !status.is_empty());
            if dirty {
                format!("{commit}+cambios")
            } else {
                commit
            }
        }
        None => "—".to_owned(),
    }
}
