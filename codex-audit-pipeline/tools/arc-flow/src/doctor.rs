use crate::project::Project;
use anyhow::{Context, Result};
use serde::Serialize;
use std::fs;
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

    for command in &project.config.doctor.required_commands {
        match command_version(command) {
            Some(version) => report.push(Level::Pass, command, version),
            None => report.push(Level::Fail, command, "command is not available on PATH"),
        }
    }

    if project.frontend.join("node_modules").is_dir() {
        report.push(Level::Pass, "frontend dependencies", "installed");
    } else {
        report.push(
            Level::Fail,
            "frontend dependencies",
            "missing; run `cd frontend && npm ci`",
        );
    }

    let database_configured = std::env::var_os("DATABASE_URL").is_some()
        || fs::read_to_string(project.backend.join(".env"))
            .map(|content| {
                content
                    .lines()
                    .any(|line| line.trim_start().starts_with("DATABASE_URL="))
            })
            .unwrap_or(false);
    if database_configured {
        report.push(
            Level::Pass,
            "runtime database",
            "DATABASE_URL is configured",
        );
    } else {
        report.push(
            Level::Fail,
            "runtime database",
            "create backend/.env from backend/.env.example",
        );
    }

    if project
        .backend
        .join("migrations")
        .read_dir()
        .map(|mut entries| {
            entries.any(|entry| {
                entry.is_ok_and(|value| value.path().extension().is_some_and(|ext| ext == "sql"))
            })
        })
        .unwrap_or(false)
    {
        report.push(Level::Pass, "migrations", "SQL migrations found");
    } else {
        report.push(Level::Fail, "migrations", "no SQL migrations found");
    }

    check_hooks(project, &mut report)?;
    check_node_version(project, &mut report)?;
    check_remotes(project, &mut report)?;
    check_test_database(project, &mut report);

    Ok(report)
}

fn command_version(command: &str) -> Option<String> {
    let output = Command::new(command).arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .map(str::to_string)
}

fn check_hooks(project: &Project, report: &mut DoctorReport) -> Result<()> {
    let output = Command::new("git")
        .current_dir(&project.root)
        .args(["config", "--get", "core.hooksPath"])
        .output()
        .context("read Git hooks path")?;
    let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let expected = &project.config.doctor.hooks_path;
    if value == *expected {
        report.push(Level::Pass, "Git hooks", "versioned hook path configured");
    } else {
        report.push(
            Level::Warn,
            "Git hooks",
            format!("run `git config core.hooksPath {expected}`"),
        );
    }
    Ok(())
}

fn check_node_version(project: &Project, report: &mut DoctorReport) -> Result<()> {
    let version_file = &project.config.doctor.node_version_file;
    let path = project.root.join(version_file);
    if !path.is_file() {
        report.push(
            Level::Warn,
            "Node version",
            format!("{version_file} is missing"),
        );
        return Ok(());
    }
    let expected = fs::read_to_string(path)?.trim().to_string();
    let actual = command_version("node")
        .unwrap_or_default()
        .trim_start_matches('v')
        .to_string();
    if actual == expected {
        report.push(Level::Pass, "Node version", expected);
    } else {
        report.push(
            Level::Fail,
            "Node version",
            format!("found {actual}, expected {expected}"),
        );
    }
    Ok(())
}

fn check_remotes(project: &Project, report: &mut DoctorReport) -> Result<()> {
    let remotes = Command::new("git")
        .current_dir(&project.root)
        .arg("remote")
        .output()
        .context("list Git remotes")?;
    let mut unsafe_remote = false;
    for remote in String::from_utf8_lossy(&remotes.stdout).lines() {
        let urls = Command::new("git")
            .current_dir(&project.root)
            .args(["remote", "get-url", "--all", remote])
            .output()?;
        unsafe_remote |= String::from_utf8_lossy(&urls.stdout).lines().any(|url| {
            (url.starts_with("https://") || url.starts_with("http://"))
                && url.split_once("//").is_some_and(|(_, tail)| {
                    tail.split_once('@')
                        .is_some_and(|(credentials, _)| !credentials.is_empty())
                })
        });
    }
    if unsafe_remote {
        report.push(
            Level::Fail,
            "Git remotes",
            "an HTTPS remote contains embedded credentials",
        );
    } else {
        report.push(Level::Pass, "Git remotes", "no embedded credentials");
    }
    Ok(())
}

fn check_test_database(project: &Project, report: &mut DoctorReport) {
    if std::env::var_os("TEST_DATABASE_URL").is_some() {
        report.push(
            Level::Pass,
            "test database",
            "TEST_DATABASE_URL is configured",
        );
        return;
    }
    if command_version("docker").is_none() {
        report.push(
            Level::Warn,
            "test database",
            "Docker is not installed; set TEST_DATABASE_URL for backend tests",
        );
        return;
    }
    let daemon = Command::new("docker")
        .current_dir(&project.root)
        .arg("info")
        .output()
        .is_ok_and(|output| output.status.success());
    if !daemon {
        report.push(
            Level::Warn,
            "test database",
            "Docker is installed but this process cannot access the daemon",
        );
        return;
    }
    let image_name = &project.config.database.image;
    let image = Command::new("docker")
        .args(["image", "inspect", image_name])
        .output()
        .is_ok_and(|output| output.status.success());
    if image {
        report.push(
            Level::Pass,
            "test database",
            format!("Docker and {image_name} ready"),
        );
    } else {
        report.push(
            Level::Warn,
            "test database",
            format!("run `docker pull {image_name}`"),
        );
    }
}
