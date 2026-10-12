//! Peak private committed memory of the generator (parent PRD, "Restricción principal"; AC-G2).
//!
//! The limit is about the peak of private committed bytes of the whole process, with the runtime
//! and every library included and the operating system's file cache excluded. Windows keeps that
//! peak itself: `PROCESS_MEMORY_COUNTERS.PeakPagefileUsage` is the highest commit charge of the
//! process since it started (what .NET calls `PeakPagedMemorySize64`). Reading it at a few
//! moments of the run is therefore exact: no peak between two readings is missed. Elsewhere the
//! figure is not available, and the run says so.
//!
//! Going over the limit never stops the run: [`MemoryWatch`] logs one warning, and the final
//! summary shows the peak against the limit.

use std::num::NonZeroU64;

use crate::Abort;
use crate::log::RunLog;
use crate::messages;

/// Bytes in a mebibyte.
pub const MIB: u64 = 1 << 20;

/// The memory limit of the generator, in MiB.
pub const DEFAULT_MEMORY_LIMIT_MIB: NonZeroU64 = NonZeroU64::new(512).unwrap();

/// `limit_mib` in bytes.
pub fn limit_bytes(limit_mib: u64) -> u64 {
    limit_mib.saturating_mul(MIB)
}

/// Peak private committed bytes of this process since it started; `None` where the operating
/// system does not report it.
#[cfg(windows)]
pub fn peak_private_bytes() -> Option<u64> {
    use windows_sys::Win32::System::ProcessStatus::{
        GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
    };
    use windows_sys::Win32::System::Threading::GetCurrentProcess;

    let size = u32::try_from(size_of::<PROCESS_MEMORY_COUNTERS>()).ok()?;
    let mut counters = PROCESS_MEMORY_COUNTERS {
        cb: size,
        ..PROCESS_MEMORY_COUNTERS::default()
    };
    // SAFETY: `GetCurrentProcess` returns a pseudo handle that needs no closing, and `counters`
    // is a writable `PROCESS_MEMORY_COUNTERS` of exactly `size` bytes that outlives the call.
    let succeeded = unsafe { GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, size) };
    if succeeded == 0 {
        return None;
    }
    u64::try_from(counters.PeakPagefileUsage).ok()
}

/// Peak private committed bytes of this process: not available outside Windows, where the
/// generator is not verified (see the PRD).
#[cfg(not(windows))]
pub fn peak_private_bytes() -> Option<u64> {
    None
}

/// Watches the peak against the limit during one run, and warns once when it goes over.
#[derive(Debug)]
pub struct MemoryWatch {
    limit_mib: NonZeroU64,
    warned: bool,
}

impl MemoryWatch {
    pub fn new(limit_mib: NonZeroU64) -> Self {
        Self {
            limit_mib,
            warned: false,
        }
    }

    /// Takes a reading of the peak: returns the warning to log the first time it is above the
    /// limit, and `None` otherwise. Reaching the limit exactly is not exceeding it.
    pub fn observe(&mut self, peak_bytes: Option<u64>) -> Option<String> {
        let peak_bytes = peak_bytes?;
        if self.warned || peak_bytes <= limit_bytes(self.limit_mib.get()) {
            return None;
        }
        self.warned = true;
        Some(messages::memory_limit_exceeded(
            peak_bytes,
            self.limit_mib.get(),
        ))
    }

    /// Reads the peak now and logs the warning if this is the first time it is over the limit.
    pub fn check(&mut self, log: &mut RunLog) -> Result<(), Abort> {
        if let Some(warning) = self.observe(peak_private_bytes()) {
            log.line(&warning)?;
        }
        Ok(())
    }

    /// Reads the peak one last time, once the dataset is published, and returns it for the
    /// summary. A warning not given yet is logged best effort: the run has already succeeded.
    pub fn final_reading(&mut self, log: &mut RunLog) -> Option<u64> {
        let peak_bytes = peak_private_bytes();
        if let Some(warning) = self.observe(peak_bytes) {
            let _ = log.line(&warning);
        }
        peak_bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limit(mib: u64) -> NonZeroU64 {
        NonZeroU64::new(mib).expect("a positive limit")
    }

    #[test]
    fn the_warning_is_given_once_the_first_time_the_peak_exceeds_the_limit() {
        let mut watch = MemoryWatch::new(limit(512));

        assert_eq!(watch.observe(Some(100 * MIB)), None);
        assert_eq!(watch.observe(Some(512 * MIB)), None, "reaching it is fine");
        assert_eq!(
            watch.observe(Some(580 * MIB)).as_deref(),
            Some(
                "Aviso: el pico de memoria privada comprometida (580 MiB) supera el límite de \
                 512 MiB."
            )
        );
        assert_eq!(watch.observe(Some(600 * MIB)), None, "only once per run");
    }

    #[test]
    fn an_unknown_peak_never_warns() {
        let mut watch = MemoryWatch::new(limit(1));

        assert_eq!(watch.observe(None), None);
        assert!(watch.observe(Some(2 * MIB)).is_some());
    }

    #[test]
    fn the_limit_in_bytes_never_overflows() {
        assert_eq!(limit_bytes(512), 536_870_912);
        assert_eq!(limit_bytes(u64::MAX), u64::MAX);
    }

    #[cfg(windows)]
    #[test]
    fn windows_reports_a_peak_that_includes_committed_allocations() {
        let before = peak_private_bytes().expect("Windows reports the peak");
        let block = std::hint::black_box(vec![1_u8; 64 * MIB as usize]);

        let after = peak_private_bytes().expect("Windows reports the peak");

        assert!(after >= before, "a peak never decreases");
        assert!(
            after >= 64 * MIB,
            "the 64 MiB block is committed memory: {after}"
        );
        drop(block);
    }

    #[cfg(not(windows))]
    #[test]
    fn the_peak_is_not_available_elsewhere() {
        assert_eq!(peak_private_bytes(), None);
    }
}
