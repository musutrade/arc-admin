use anyhow::{Context, Result};
use serde::Serialize;
use std::ffi::{OsStr, OsString};
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

static CANCELLED: AtomicBool = AtomicBool::new(false);

#[cfg(unix)]
extern "C" fn handle_signal(_signal: libc::c_int) {
    CANCELLED.store(true, Ordering::SeqCst);
}

pub fn install_signal_handlers() {
    #[cfg(unix)]
    unsafe {
        let handler = handle_signal as *const () as libc::sighandler_t;
        libc::signal(libc::SIGINT, handler);
        libc::signal(libc::SIGTERM, handler);
    }
}

pub fn cancelled() -> bool {
    CANCELLED.load(Ordering::SeqCst)
}

#[derive(Debug)]
pub struct Task {
    pub label: String,
    program: OsString,
    args: Vec<OsString>,
    cwd: PathBuf,
    env: Vec<(OsString, OsString)>,
    timeout: Duration,
    log: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskResult {
    pub label: String,
    pub passed: bool,
    pub timed_out: bool,
    pub cancelled: bool,
    pub duration_ms: u128,
    pub log: String,
    pub detail: Option<String>,
}

impl Task {
    pub fn new(
        label: impl Into<String>,
        program: impl AsRef<OsStr>,
        cwd: &Path,
        log: PathBuf,
    ) -> Self {
        Self {
            label: label.into(),
            program: program.as_ref().to_os_string(),
            args: Vec::new(),
            cwd: cwd.to_path_buf(),
            env: Vec::new(),
            timeout: Duration::from_secs(180),
            log,
        }
    }

    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.args
            .extend(args.into_iter().map(|arg| arg.as_ref().to_os_string()));
        self
    }

    pub fn env(mut self, key: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> Self {
        self.env
            .push((key.as_ref().to_os_string(), value.as_ref().to_os_string()));
        self
    }

    pub fn timeout(mut self, seconds: u64) -> Self {
        self.timeout = Duration::from_secs(seconds);
        self
    }

    pub fn run(self) -> Result<TaskResult> {
        if let Some(parent) = self.log.parent() {
            fs::create_dir_all(parent)?;
        }
        let stdout = File::create(&self.log)
            .with_context(|| format!("create log {}", self.log.display()))?;
        let stderr = stdout.try_clone()?;
        let started = Instant::now();
        let mut command = Command::new(&self.program);
        command
            .args(&self.args)
            .current_dir(&self.cwd)
            .envs(self.env)
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr));
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        let mut child = command
            .spawn()
            .with_context(|| format!("start {}", self.program.to_string_lossy()))?;

        let (status, timed_out, was_cancelled) = loop {
            if let Some(status) = child.try_wait()? {
                break (status, false, false);
            }
            if cancelled() {
                break (terminate(&mut child)?, false, true);
            }
            if started.elapsed() >= self.timeout {
                break (terminate(&mut child)?, true, false);
            }
            thread::sleep(Duration::from_millis(100));
        };

        Ok(TaskResult {
            label: self.label,
            passed: status.success() && !timed_out && !was_cancelled,
            timed_out,
            cancelled: was_cancelled,
            duration_ms: started.elapsed().as_millis(),
            log: self.log.to_string_lossy().to_string(),
            detail: if was_cancelled {
                Some("cancelled".to_string())
            } else if timed_out {
                Some("timed out".to_string())
            } else if status.success() {
                None
            } else {
                status.code().map(|code| format!("exit code {code}"))
            },
        })
    }
}

#[cfg(unix)]
fn terminate(child: &mut Child) -> std::io::Result<ExitStatus> {
    let process_group = -(child.id() as i32);
    // The child is its process-group leader, so this also stops spawned test/build processes.
    unsafe {
        libc::kill(process_group, libc::SIGTERM);
    }
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        if let Some(status) = child.try_wait()? {
            return Ok(status);
        }
        thread::sleep(Duration::from_millis(50));
    }
    unsafe {
        libc::kill(process_group, libc::SIGKILL);
    }
    child.wait()
}

#[cfg(not(unix))]
fn terminate(child: &mut Child) -> std::io::Result<ExitStatus> {
    child.kill()?;
    child.wait()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn timeout_terminates_the_task() {
        let log = std::env::temp_dir().join(format!("arc-flow-timeout-{}.log", std::process::id()));
        let result = Task::new("timeout fixture", "sleep", Path::new("."), log.clone())
            .args(["5"])
            .timeout(0)
            .run()
            .expect("run timeout fixture");

        assert!(!result.passed);
        assert!(result.timed_out);
        let _ = fs::remove_file(log);
    }
}
