//! Command line entry point of the CanopyDB query app: the original query app's program
//! ([`crime_query::main`]) with the CanopyDB [`crime_canopy_query::APP`]. Exit codes as the
//! original's (decision C6): 0 completed, 1 rejected or aborted, 2 invalid command line, 130
//! cancelled with Ctrl+C.

use std::process::ExitCode;

fn main() -> ExitCode {
    ExitCode::from(crime_query::main(&crime_canopy_query::APP))
}
