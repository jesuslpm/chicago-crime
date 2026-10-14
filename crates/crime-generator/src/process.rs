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

/// Whether the process with ID `process_id` is running.
///
/// On Linux a process that has ended has no `/proc/<pid>` entry, or is a zombie (`Z`) or dead
/// (`X`) one waiting for its parent. Any other answer, or a doubt, counts as alive. A generator
/// in another PID namespace (another container sharing the temporary folder) cannot be seen from
/// here; the operator gives each one its own temporary folder.
#[cfg(target_os = "linux")]
pub fn is_alive(process_id: u32) -> bool {
    match std::fs::read_to_string(format!("/proc/{process_id}/stat")) {
        Ok(stat) => !matches!(process_state(&stat), Some('Z' | 'X' | 'x')),
        Err(error) => error.kind() != std::io::ErrorKind::NotFound,
    }
}

/// Whether the process with ID `process_id` is running: always `true` on the remaining systems,
/// so stale sort folders are never deleted there.
#[cfg(not(any(windows, target_os = "linux")))]
pub fn is_alive(_process_id: u32) -> bool {
    true
}

/// The state letter of a Linux `/proc/<pid>/stat` line. It follows the command name, which is in
/// parentheses and may itself hold spaces and parentheses, so the last `)` ends it.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn process_state(stat: &str) -> Option<char> {
    stat.rsplit_once(')')?.1.trim_start().chars().next()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_state_is_the_letter_after_the_command_name() {
        assert_eq!(process_state("42 (crime-generator) S 1 42 42 0"), Some('S'));
        assert_eq!(
            process_state("43 (a) b (c)) Z 1 43"),
            Some('Z'),
            "parentheses in the name"
        );
        assert_eq!(process_state("44 (no end"), None);
        assert_eq!(process_state("45 (empty)"), None);
    }
}
