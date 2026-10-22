//! Running the real apps: start one with its phase markers, follow its phases, kill it or press
//! Ctrl+C, and wait for it (canopy-db-robustez PRD, "Reglas comunes").
//!
//! - **Kill** is `TerminateProcess` on Windows and `SIGKILL` on Linux: [`std::process::Child::kill`].
//! - **Ctrl+C** is the signal the app honors. On Linux it is `SIGINT`. On Windows a console Ctrl+C
//!   reaches every process of a console, so the app runs inside a hidden console of its own: this
//!   binary starts itself as a host there ([`run_ctrl_c_host`]), the host starts the app and, when
//!   asked through a trigger file, sends the event to its console while ignoring it itself.
//! - **Phases** come from the apps' hidden phase markers ([`crime_dataset::probe`]): each run has a
//!   phase file, and a pause can stretch one phase so that a signal lands in it.

use std::ffi::OsString;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crime_dataset::probe::{PHASE_FILE_ENV, PHASE_PAUSE_ENV};

/// The hidden first argument that turns this binary into a Ctrl+C host (Windows only).
pub const CTRL_C_HOST_ARG: &str = "--ctrl-c-host";

/// How often a wait looks again.
const POLL: Duration = Duration::from_millis(10);

/// What to start.
#[derive(Debug, Clone)]
pub struct Launch {
    /// The binary.
    pub program: PathBuf,
    pub args: Vec<OsString>,
    /// Pause `(phase, milliseconds)` right after that phase is marked.
    pub pause: Option<(String, u64)>,
    /// Whether the run may receive a Ctrl+C: on Windows it then runs in a console of its own.
    pub ctrl_c: bool,
    /// The folder where the run keeps its phase file, stdout and stderr.
    pub run_dir: PathBuf,
}

impl Launch {
    pub fn new(program: &Path, run_dir: &Path) -> Self {
        Self {
            program: program.to_path_buf(),
            args: Vec::new(),
            pause: None,
            ctrl_c: false,
            run_dir: run_dir.to_path_buf(),
        }
    }

    pub fn arg(mut self, arg: impl Into<OsString>) -> Self {
        self.args.push(arg.into());
        self
    }

    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    pub fn pause(mut self, phase: &str, millis: u64) -> Self {
        self.pause = Some((phase.to_owned(), millis));
        self
    }

    pub fn with_ctrl_c(mut self) -> Self {
        self.ctrl_c = true;
        self
    }

    /// Starts the run.
    pub fn start(self) -> io::Result<Running> {
        fs::create_dir_all(&self.run_dir)?;
        let phases = self.run_dir.join("phases.txt");
        let stdout = self.run_dir.join("stdout.txt");
        let stderr = self.run_dir.join("stderr.txt");
        let trigger = self.run_dir.join("ctrl-c.trigger");
        for file in [&phases, &stdout, &stderr, &trigger] {
            let _ = fs::remove_file(file);
        }
        let mut command = if self.ctrl_c && cfg!(windows) {
            let mut host = Command::new(std::env::current_exe()?);
            host.arg(CTRL_C_HOST_ARG)
                .arg(&trigger)
                .arg(&stdout)
                .arg(&stderr)
                .arg(&self.program)
                .args(&self.args);
            host.stdout(Stdio::null()).stderr(Stdio::null());
            detach(&mut host);
            host
        } else {
            let mut app = Command::new(&self.program);
            app.args(&self.args)
                .stdout(File::create(&stdout)?)
                .stderr(File::create(&stderr)?);
            app
        };
        command.stdin(Stdio::null()).env(PHASE_FILE_ENV, &phases);
        match &self.pause {
            Some((phase, millis)) => command.env(PHASE_PAUSE_ENV, format!("{phase}:{millis}")),
            None => command.env_remove(PHASE_PAUSE_ENV),
        };
        let child = command.spawn()?;
        Ok(Running {
            child,
            hosted: self.ctrl_c && cfg!(windows),
            phases,
            stdout,
            stderr,
            trigger,
            started: Instant::now(),
            signalled: None,
            ended: None,
        })
    }
}

/// How a run ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ended {
    /// The exit code, `None` when a signal ended it (a kill on Linux).
    pub code: Option<i32>,
    /// From the start of the run.
    pub after_start: Duration,
    /// From the kill or the Ctrl+C, if one was sent.
    pub after_signal: Option<Duration>,
}

/// A run in progress.
#[derive(Debug)]
pub struct Running {
    child: Child,
    /// Whether `child` is the Ctrl+C host of the app rather than the app itself.
    hosted: bool,
    phases: PathBuf,
    stdout: PathBuf,
    stderr: PathBuf,
    trigger: PathBuf,
    started: Instant,
    signalled: Option<Instant>,
    ended: Option<Ended>,
}

impl Running {
    /// The phases the app has marked so far, in order.
    pub fn phases(&self) -> Vec<String> {
        fs::read_to_string(&self.phases)
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect()
    }

    /// The last phase marked so far.
    pub fn last_phase(&self) -> Option<String> {
        self.phases().pop()
    }

    /// Waits until the app marks `phase`; `false` if it ends first or `timeout` passes.
    pub fn wait_phase(&mut self, phase: &str, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        loop {
            if self.phases().iter().any(|marked| marked == phase) {
                return true;
            }
            if self.has_ended() || Instant::now() >= deadline {
                return false;
            }
            thread::sleep(POLL);
        }
    }

    /// Waits until `condition` holds; `false` if the app ends first or `timeout` passes.
    pub fn wait_until(&mut self, timeout: Duration, condition: impl Fn(&Running) -> bool) -> bool {
        let deadline = Instant::now() + timeout;
        loop {
            if condition(self) {
                return true;
            }
            if self.has_ended() || Instant::now() >= deadline {
                return false;
            }
            thread::sleep(POLL);
        }
    }

    /// Whether the app has ended.
    pub fn has_ended(&mut self) -> bool {
        self.poll().is_some()
    }

    fn poll(&mut self) -> Option<Ended> {
        if self.ended.is_none()
            && let Ok(Some(status)) = self.child.try_wait()
        {
            let now = Instant::now();
            self.ended = Some(Ended {
                code: status.code(),
                after_start: now - self.started,
                after_signal: self.signalled.map(|at| now - at),
            });
        }
        self.ended
    }

    /// Kills the app: `TerminateProcess` on Windows, `SIGKILL` on Linux. Returns whether it was
    /// still running when the kill was sent. A hosted run cannot be killed.
    pub fn kill(&mut self) -> bool {
        assert!(!self.hosted, "a run started for Ctrl+C is not killed");
        if self.has_ended() {
            return false;
        }
        self.signalled = Some(Instant::now());
        let _ = self.child.kill();
        true
    }

    /// Presses Ctrl+C in the app. Returns whether it was still running when it was sent.
    pub fn ctrl_c(&mut self) -> io::Result<bool> {
        if self.has_ended() {
            return Ok(false);
        }
        self.signalled = Some(Instant::now());
        if self.hosted {
            File::create(&self.trigger)?;
        } else {
            send_sigint(&mut self.child)?;
        }
        Ok(true)
    }

    /// Waits for the end of the app at most `timeout`; `None` if it is still running.
    pub fn wait(&mut self, timeout: Duration) -> Option<Ended> {
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(ended) = self.poll() {
                return Some(ended);
            }
            if Instant::now() >= deadline {
                return None;
            }
            thread::sleep(POLL);
        }
    }

    /// The app's standard output so far.
    pub fn stdout(&self) -> String {
        String::from_utf8_lossy(&fs::read(&self.stdout).unwrap_or_default()).into_owned()
    }

    /// How many bytes the app has written to its standard output so far.
    pub fn stdout_len(&self) -> u64 {
        fs::metadata(&self.stdout).map_or(0, |meta| meta.len())
    }

    /// The app's standard error so far.
    pub fn stderr(&self) -> String {
        String::from_utf8_lossy(&fs::read(&self.stderr).unwrap_or_default()).into_owned()
    }

    /// When the run started.
    pub fn started(&self) -> Instant {
        self.started
    }

    /// The peak private committed memory of the app, in bytes, as the acceptance checks measure
    /// it (`PeakPagefileUsage`, also once it has ended); `None` where the system does not tell
    /// (Linux) or for a hosted run.
    pub fn peak_private_bytes(&self) -> Option<u64> {
        if self.hosted {
            return None;
        }
        peak_private_bytes(&self.child)
    }
}

#[cfg(windows)]
fn peak_private_bytes(child: &Child) -> Option<u64> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::System::ProcessStatus::{
        GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
    };
    // SAFETY: the handle belongs to `child`, which is alive while borrowed; the counters are
    // plain data of the size given.
    unsafe {
        let mut counters: PROCESS_MEMORY_COUNTERS = std::mem::zeroed();
        let size = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
        (GetProcessMemoryInfo(child.as_raw_handle(), &mut counters, size) != 0)
            .then_some(counters.PeakPagefileUsage as u64)
    }
}

#[cfg(not(windows))]
fn peak_private_bytes(_child: &Child) -> Option<u64> {
    None
}

impl Drop for Running {
    fn drop(&mut self) {
        // Never leave an app behind: a scenario that gives up on a run kills it.
        if self.ended.is_none() && matches!(self.child.try_wait(), Ok(None)) {
            if self.hosted {
                let _ = File::create(&self.trigger);
                let _ = self.child.wait();
            } else {
                let _ = self.child.kill();
                let _ = self.child.wait();
            }
        }
    }
}

/// Runs the app to its end and returns its exit code, stdout and stderr.
pub fn run_to_end(launch: Launch, timeout: Duration) -> io::Result<(Option<i32>, String, String)> {
    let mut running = launch.start()?;
    match running.wait(timeout) {
        Some(ended) => Ok((ended.code, running.stdout(), running.stderr())),
        None => Err(io::Error::new(
            io::ErrorKind::TimedOut,
            format!("la ejecución no terminó en {} s", timeout.as_secs()),
        )),
    }
}

#[cfg(unix)]
fn send_sigint(child: &mut Child) -> io::Result<()> {
    let status = Command::new("kill")
        .arg("-INT")
        .arg(child.id().to_string())
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other("kill -INT failed"))
    }
}

#[cfg(windows)]
fn send_sigint(_child: &mut Child) -> io::Result<()> {
    Err(io::Error::other(
        "a run that receives Ctrl+C must be started with with_ctrl_c",
    ))
}

#[cfg(windows)]
fn detach(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    command.creation_flags(DETACHED_PROCESS);
}

#[cfg(not(windows))]
fn detach(_command: &mut Command) {}

/// The Ctrl+C host (Windows): `--ctrl-c-host <trigger> <stdout> <stderr> <program> [args…]`.
/// Gets a hidden console of its own, starts the program in it with its output in the two files,
/// waits for the trigger file, sends Ctrl+C to the console while ignoring it, and ends with the
/// program's exit code.
#[cfg(windows)]
pub fn run_ctrl_c_host(args: &[OsString]) -> i32 {
    use windows_sys::Win32::System::Console::{
        AllocConsole, CTRL_C_EVENT, FreeConsole, GenerateConsoleCtrlEvent, GetConsoleWindow,
        SetConsoleCtrlHandler,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{SW_HIDE, ShowWindow};

    let [trigger, stdout, stderr, program, rest @ ..] = args else {
        return 2;
    };
    // SAFETY: plain Win32 calls on this process's own console.
    unsafe {
        FreeConsole();
        if AllocConsole() == 0 {
            return 3;
        }
        let window = GetConsoleWindow();
        if !window.is_null() {
            ShowWindow(window, SW_HIDE);
        }
        // The runner may have been started with Ctrl+C ignored, which every child inherits:
        // turn it back on so that the program honors it.
        SetConsoleCtrlHandler(None, 0);
    }
    let (Ok(out), Ok(err)) = (File::create(stdout), File::create(stderr)) else {
        return 3;
    };
    let mut child = match Command::new(program)
        .args(rest)
        .stdin(Stdio::null())
        .stdout(out)
        .stderr(err)
        .spawn()
    {
        Ok(child) => child,
        Err(_) => return 3,
    };
    let trigger = Path::new(trigger);
    loop {
        if let Ok(Some(status)) = child.try_wait() {
            return status.code().unwrap_or(3);
        }
        if trigger.exists() {
            break;
        }
        thread::sleep(Duration::from_millis(2));
    }
    // SAFETY: ignore Ctrl+C in this process, then send it to every process of this console.
    unsafe {
        SetConsoleCtrlHandler(None, 1);
        GenerateConsoleCtrlEvent(CTRL_C_EVENT, 0);
    }
    match child.wait() {
        Ok(status) => status.code().unwrap_or(3),
        Err(_) => 3,
    }
}

#[cfg(not(windows))]
pub fn run_ctrl_c_host(_args: &[OsString]) -> i32 {
    2
}
