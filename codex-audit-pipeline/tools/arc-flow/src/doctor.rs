use crate::config::{DoctorCheck, DoctorCheckKind, PathType};
use crate::project::Project;
use anyhow::{bail, Context, Result};
use globset::{Glob, GlobSetBuilder};
use ignore::WalkBuilder;
use serde::Serialize;
use std::fs;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
enum Level {
    Pass,
    Warn,
    Fail,
}

#[derive(Debug, Serialize)]
struct Check {
    level: Level,
    name: String,
    detail: String,
}

#[derive(Debug, Serialize)]
pub struct DoctorReport {
    project_root: String,
    checks: Vec<Check>,
    pub failures: usize,
    pub warnings: usize,
}

impl DoctorReport {
    fn new(project: &Project) -> Self {
        Self {
            project_root: project.root.to_string_lossy().to_string(),
            checks: Vec::new(),
            failures: 0,
            warnings: 0,
        }
    }

    fn push(&mut self, level: Level, name: impl Into<String>, detail: impl Into<String>) {
        match level {
            Level::Fail => self.failures += 1,
            Level::Warn => self.warnings += 1,
            Level::Pass => {}
        }
        self.checks.push(Check {
            level,
            name: name.into(),
            detail: detail.into(),
        });
    }

    pub fn print(&self) {
        println!("arc-flow doctor");
        println!("Project: {}\n", self.project_root);
        for check in &self.checks {
            let marker = match check.level {
                Level::Pass => "PASS",
                Level::Warn => "WARN",
                Level::Fail => "FAIL",
            };
            println!("[{marker}] {:<22} {}", check.name, check.detail);
        }
        println!(
            "\nSummary: {} failure(s), {} warning(s)",
            self.failures, self.warnings
        );
    }
}

pub fn run(project: &Project) -> Result<DoctorReport> {
    let mut report = DoctorReport::new(project);
    for check in &project.config.doctor.checks {
        match run_check(project, check) {
            Ok(detail) => report.push(Level::Pass, &check.label, detail),
            Err(error) => {
                let level = if check.required {
                    Level::Fail
                } else {
                    Level::Warn
                };
                let detail = match &check.help {
                    Some(help) => format!("{error:#}; {help}"),
                    None => format!("{error:#}"),
                };
                report.push(level, &check.label, detail);
            }
        }
    }
    Ok(report)
}

fn run_check(project: &Project, check: &DoctorCheck) -> Result<String> {
    match &check.kind {
        DoctorCheckKind::Command { program, args } => {
            let args = args
                .iter()
                .map(|arg| project.expand(arg))
                .collect::<Vec<_>>();
            command_output(program, &args)
        }
        DoctorCheckKind::Path { path, path_type } => {
            let path = project.expand(path);
            let path = Path::new(&path);
            let exists = match path_type {
                PathType::Any => path.exists(),
                PathType::File => path.is_file(),
                PathType::Directory => path.is_dir(),
            };
            if !exists {
                bail!("{} is missing", path.display());
            }
            Ok(path.display().to_string())
        }
        DoctorCheckKind::Glob { pattern } => check_glob(project, pattern),
        DoctorCheckKind::Env { name } => {
            std::env::var_os(name).ok_or_else(|| anyhow::anyhow!("{name} is not configured"))?;
            Ok(format!("{name} is configured"))
        }
        DoctorCheckKind::EnvOrFile {
            env,
            path,
            contains,
        } => {
            if std::env::var_os(env).is_some() {
                return Ok(format!("{env} is configured"));
            }
            let path = project.expand(path);
            let found = fs::read_to_string(&path)
                .map(|content| {
                    content
                        .lines()
                        .any(|line| line.trim_start().starts_with(contains))
                })
                .unwrap_or(false);
            if !found {
                bail!("{env} is absent and {path} does not define {contains}");
            }
            Ok(format!("{contains} found in {path}"))
        }
        DoctorCheckKind::GitConfig { key, expected } => {
            let output = Command::new("git")
                .current_dir(&project.root)
                .args(["config", "--get", key])
                .output()
                .with_context(|| format!("read Git config {key}"))?;
            let actual = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if actual != *expected {
                bail!("Git config {key} is {actual:?}, expected {expected:?}");
            }
            Ok(format!("{key}={expected}"))
        }
        DoctorCheckKind::GitRemotes => check_remotes(project),
        DoctorCheckKind::Version {
            program,
            args,
            path,
            trim_prefix,
        } => {
            let args = args
                .iter()
                .map(|arg| project.expand(arg))
                .collect::<Vec<_>>();
            let actual = command_output(program, &args)?
                .trim_start_matches(trim_prefix)
                .to_string();
            let path = project.expand(path);
            let expected = fs::read_to_string(&path)
                .with_context(|| format!("read version file {path}"))?
                .trim()
                .to_string();
            if actual != expected {
                bail!("found {actual}, expected {expected}");
            }
            Ok(expected)
        }
        DoctorCheckKind::Service { service } => crate::service::check_available(project, service),
    }
}

fn command_output(program: &str, args: &[String]) -> Result<String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .with_context(|| format!("command {program} is not available on PATH"))?;
    if !output.status.success() {
        bail!("command {program} exited with {}", output.status);
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .map(str::to_string)
        .filter(|line| !line.is_empty())
        .ok_or_else(|| anyhow::anyhow!("command {program} produced no output"))
}

fn check_glob(project: &Project, pattern: &str) -> Result<String> {
    let pattern = project.expand(pattern);
    let mut builder = GlobSetBuilder::new();
    builder.add(Glob::new(&pattern)?);
    let matcher = builder.build()?;
    let found = WalkBuilder::new(&project.root)
        .hidden(false)
        .build()
        .filter_map(Result::ok)
        .any(|entry| {
            entry.file_type().is_some_and(|kind| kind.is_file()) && matcher.is_match(entry.path())
        });
    if !found {
        bail!("no files match {pattern}");
    }
    Ok(format!("matched {pattern}"))
}

fn check_remotes(project: &Project) -> Result<String> {
    let remotes = Command::new("git")
        .current_dir(&project.root)
        .arg("remote")
        .output()
        .context("list Git remotes")?;
    for remote in String::from_utf8_lossy(&remotes.stdout).lines() {
        let urls = Command::new("git")
            .current_dir(&project.root)
            .args(["remote", "get-url", "--all", remote])
            .output()?;
        let unsafe_url = String::from_utf8_lossy(&urls.stdout).lines().any(|url| {
            (url.starts_with("https://") || url.starts_with("http://"))
                && url.split_once("//").is_some_and(|(_, tail)| {
                    tail.split_once('@')
                        .is_some_and(|(credentials, _)| !credentials.is_empty())
                })
        });
        if unsafe_url {
            bail!("remote {remote:?} contains embedded HTTPS credentials");
        }
    }
    Ok("no embedded credentials".into())
}
