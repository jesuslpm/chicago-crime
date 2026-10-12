//! Cancellation by the operator with Ctrl+C, treated as a failed generation (child flow, step 4;
//! AC-G7).
//!
//! The console handler does nothing but raise a [`CancelFlag`]. The run checks the flag at a few
//! points: for every row read, for every entry of both merges while the tables are written, and
//! right before `CURRENT` is replaced. A raised flag aborts the run with [`Abort::Cancelled`], so
//! the usual cleanup of an abort applies: the new version folder, both sort folders and
//! `CURRENT.tmp` are deleted by their owners, and the previous dataset is never touched. Once
//! `CURRENT` has been replaced the run has succeeded, and a later Ctrl+C changes nothing.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::Abort;
use crate::fail::{self, FailPoint};

/// Whether the operator asked the run to stop. Clones share the same flag.
#[derive(Debug, Clone, Default)]
pub struct CancelFlag(Arc<AtomicBool>);

impl CancelFlag {
    /// A flag that is not raised.
    pub fn new() -> Self {
        Self::default()
    }

    /// Asks the run to stop at its next check point.
    pub fn raise(&self) {
        // Nothing else is published through the flag, so no ordering with other memory is needed.
        self.0.store(true, Ordering::Relaxed);
    }

    pub fn is_raised(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }

    /// Makes Ctrl+C raise this flag instead of ending the process (on Windows, Ctrl+Break and
    /// closing the console as well; closing the console still ends the process shortly after).
    /// Only one handler can be installed per process.
    pub fn raise_on_ctrl_c(&self) -> Result<(), ctrlc::Error> {
        let flag = self.clone();
        ctrlc::set_handler(move || flag.raise())
    }

    /// Fails with [`Abort::Cancelled`] once the flag is raised.
    ///
    /// `point` names this check for the tests: with the `fail-points` feature, a test that armed
    /// it raises the flag right here, exactly as a Ctrl+C at this moment would.
    pub(crate) fn check(&self, point: FailPoint) -> Result<(), Abort> {
        if fail::check(point).is_err() {
            self.raise();
        }
        if self.is_raised() {
            Err(Abort::Cancelled)
        } else {
            Ok(())
        }
    }
}
