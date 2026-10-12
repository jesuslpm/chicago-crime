//! Deterministic failure injection for the tests.
//!
//! The generator calls [`check`] at a few named points of a run. In ordinary builds `check` does
//! nothing. With the `fail-points` feature, which only this crate's own tests enable (through a
//! dev-dependency of the crate on itself), a test can [`arm`] one point on its own thread; the next
//! time the run reaches that point on that thread, `check` returns an I/O error once, exactly as a
//! real failure there would, and the point is disarmed.
//!
//! Points are armed per thread, so tests running in parallel never see each other's failures.

use std::io;

/// A place in the run where a test can inject a failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailPoint {
    /// A leftover of an interrupted run is about to be deleted, before the new version exists.
    LeftoverRemoval,
    /// The new version folder was just created; no table exists yet.
    VersionCreated,
    /// The primary table is being written while the index sort receives entries: both sorts hold
    /// temporary folders.
    BothSortsAlive,
    /// Every table is written and synced; the manifest and the switch have not happened.
    TablesWritten,
    /// `CURRENT.tmp` names the new version and is synced; it has not replaced `CURRENT` yet.
    BeforeSwitch,
    /// The new version is published; the previous version folder is about to be deleted.
    PreviousVersionRemoval,
}

/// Fails once if a test armed `point` on this thread.
#[cfg(feature = "fail-points")]
pub(crate) fn check(point: FailPoint) -> io::Result<()> {
    let fired = ARMED.with(|armed| {
        if armed.get() == Some(point) {
            armed.set(None);
            true
        } else {
            false
        }
    });
    if fired {
        Err(io::Error::other(format!("injected failure at {point:?}")))
    } else {
        Ok(())
    }
}

/// Does nothing: fail points exist only in test builds.
#[cfg(not(feature = "fail-points"))]
#[inline(always)]
pub(crate) fn check(_point: FailPoint) -> io::Result<()> {
    Ok(())
}

#[cfg(feature = "fail-points")]
thread_local! {
    static ARMED: std::cell::Cell<Option<FailPoint>> = const { std::cell::Cell::new(None) };
}

/// Arms `point` on the current thread until it fires once or the returned guard is dropped.
#[cfg(feature = "fail-points")]
pub fn arm(point: FailPoint) -> Armed {
    ARMED.with(|armed| armed.set(Some(point)));
    Armed { _private: () }
}

/// Keeps a fail point armed; dropping it disarms the point if it has not fired, so it never
/// leaks into a later run on the same thread.
#[cfg(feature = "fail-points")]
#[must_use = "the fail point is disarmed when the guard is dropped"]
pub struct Armed {
    _private: (),
}

#[cfg(feature = "fail-points")]
impl Drop for Armed {
    fn drop(&mut self) {
        ARMED.with(|armed| armed.set(None));
    }
}
