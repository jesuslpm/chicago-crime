//! Whether another process is still running, to tell the sort folders of a killed generator from
//! those of a running one (see [`crate::sort::discard_stale_sort_dirs`]).
//!
//! Every doubt is answered "alive": a folder of a running process must never be deleted, while a
//! stale folder kept by mistake only costs disk space until a later run.

/// Whether the process with ID `process_id` is running.
///
/// On Windows the process is opened with the least access right that can read its exit code.
/// No process with that ID means it has ended. A process that cannot be opened for any other
/// reason (one of another user, for example) exists, so it counts as alive, and so does one that
/// ended with the exit code `STILL_ACTIVE` (259), which Windows cannot tell from a running one.
#[cfg(windows)]
pub fn is_alive(process_id: u32) -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, ERROR_INVALID_PARAMETER, STILL_ACTIVE};
    use windows_sys::Win32::System::Threading::{
        GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    // SAFETY: `OpenProcess` takes plain values and returns a handle or null.
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, process_id) };
    if handle.is_null() {
        let no_such_process = std::io::Error::last_os_error()
            .raw_os_error()
            .is_some_and(|code| u32::try_from(code) == Ok(ERROR_INVALID_PARAMETER));
        return !no_such_process;
    }
    let mut exit_code = 0_u32;
    // SAFETY: `handle` is a valid process handle opened above with the right to query its exit
    // code, and `exit_code` is a writable `u32` that outlives the call.
    let queried = unsafe { GetExitCodeProcess(handle, &mut exit_code) };
    // SAFETY: `handle` is valid and closed exactly once, here; it is not used afterwards.
    unsafe { CloseHandle(handle) };
    queried == 0 || u32::try_from(STILL_ACTIVE) == Ok(exit_code)
}

/// Whether the process with ID `process_id` is running: always `true` outside Windows, where the
/// generator is not verified (see the PRD), so stale sort folders are never deleted there.
#[cfg(not(windows))]
pub fn is_alive(_process_id: u32) -> bool {
    true
}
