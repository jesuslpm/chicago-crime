//! R3–R7: the CanopyDB query app killed or cancelled midway, and the lock it leaves.
//!
//! R5 is the model of the others: launch, wait for the phase, send the signal, check that it
//! landed there, then check the variant's expectations, each one a named check.

use std::fs;
use std::time::Duration;

use super::{Landed, Scenario, Systems, failed_to_run, next_query_opens, output_temporaries};
use crate::context::{Context, QUERY_TIMEOUT};
use crate::report::VariantResult;

pub fn scenarios() -> Vec<Scenario> {
    vec![Scenario {
        id: "R5",
        systems: Systems::Both,
        racy: false,
        variants: vec![
            ("consultor", R5_CHECKS.to_vec()),
            ("lote", R5_CHECKS.to_vec()),
        ],
        run: r5,
    }]
}

const R5_CHECKS: [&str; 6] = [
    "codigo",
    "mensaje",
    "tiempo",
    "salida",
    "siguiente-sin-espera",
    "siguiente-resultado",
];

/// How long after F11 starts the Ctrl+C is sent: F11 lists every `THEFT` and lasts seconds.
const INTO_F11: Duration = Duration::from_millis(400);

/// R5: Ctrl+C midway through F11, outside the wait, to a single query and to a batch: 130 and
/// `Consulta cancelada.` in less than 1 s, no output file nor temporary left, and the next query
/// app opens without the wait warning.
fn r5(context: &Context) -> Vec<VariantResult> {
    let dataset = match context.reference_dataset() {
        Ok(dataset) => dataset,
        Err(error) => {
            return ["consultor", "lote"]
                .iter()
                .map(|variant| failed_to_run(&format!("R5/{variant}"), &error))
                .collect();
        }
    };
    ["consultor", "lote"]
        .iter()
        .map(|variant| {
            let id = format!("R5/{variant}");
            r5_variant(context, &dataset, &id, *variant == "lote").unwrap_or_else(|result| *result)
        })
        .collect()
}

fn r5_variant(
    context: &Context,
    dataset: &std::path::Path,
    id: &str,
    batch: bool,
) -> Result<VariantResult, Box<VariantResult>> {
    let mut result = VariantResult::new(id);
    let landed = super::until_landed(id, |_| {
        let run_dir = context.run_dir(id);
        fs::create_dir_all(&run_dir)?;
        let output_name = "f11.csv";
        let output = run_dir.join(output_name);
        let f11 = format!(
            "{} --output \"{}\"",
            context.fixed_line(11),
            output.display()
        );
        // The batch runs F1 to F10 first, then F11 with its output file.
        let launch = if batch {
            let mut lines: Vec<String> = (1..=10)
                .map(|number| context.fixed_line(number).to_owned())
                .collect();
            lines.push(f11);
            context.batch(dataset, &(lines.join("\n") + "\n"), id)?
        } else {
            let mut args = context.fixed_args(11);
            args.push("--output".to_owned());
            args.push(output.display().to_string());
            context.query(dataset, &args, id)
        };
        let mut running = launch.with_ctrl_c().start()?;
        let queries_before_f11 = if batch { 11 } else { 1 };
        let reached = running.wait_until(QUERY_TIMEOUT, |run| {
            run.phases()
                .iter()
                .filter(|phase| *phase == "query")
                .count()
                >= queries_before_f11
        });
        if !reached {
            return Ok(Landed::Missed("la consulta F11 no empezó".to_owned()));
        }
        std::thread::sleep(INTO_F11);
        if !running.ctrl_c()? {
            return Ok(Landed::Missed("F11 ya había terminado".to_owned()));
        }
        let ended = running
            .wait(QUERY_TIMEOUT)
            .ok_or_else(|| std::io::Error::other("el consultor no terminó tras Ctrl+C"))?;
        Ok(Landed::Yes((
            ended,
            running.stderr(),
            run_dir,
            output_name.to_owned(),
        )))
    });
    let (ended, stderr, run_dir, output_name) = landed.map_err(Box::new)?;
    let after = ended.after_signal.unwrap_or_default();
    result.observe(format!(
        "código {:?} a los {:.2} s del Ctrl+C",
        ended.code,
        after.as_secs_f64()
    ));
    result.check_eq("codigo", Some(130), ended.code);
    result.check(
        "mensaje",
        stderr.lines().any(|line| line == "Consulta cancelada."),
        format!("falta `Consulta cancelada.` en: {}", stderr.trim()),
    );
    result.check(
        "tiempo",
        after < Duration::from_secs(1),
        format!("tardó {:.2} s", after.as_secs_f64()),
    );
    let leftovers = output_temporaries(&run_dir, &output_name);
    let file_left = run_dir.join(&output_name).exists();
    result.check(
        "salida",
        !file_left && leftovers.is_empty(),
        format!("quedó el archivo de salida: {file_left}; temporales: {leftovers:?}"),
    );
    if let Err(error) = next_query_opens(context, &mut result, dataset) {
        return Err(Box::new(failed_to_run(id, &error)));
    }
    Ok(result)
}
