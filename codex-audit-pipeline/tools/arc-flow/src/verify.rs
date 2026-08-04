use crate::audit;
use crate::process::{Task, TaskResult};
use crate::project::Project;
use crate::scope::{Component, ScopeResult};
use crate::secrets::{self, SecretMode};
use anyhow::{bail, Context, Result};
use regex::Regex;
use serde::Serialize;
use std::collections::BTreeSet;
use std::fs;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const POSTGRES_IMAGE: &str = "postgres:16-alpine";

#[derive(Debug, Clone, Copy)]
pub enum Profile {
    Full,
    Hook,
}

#[derive(Debug, Serialize)]
pub struct VerificationReport {
    pub timestamp: String,
    pub profile: String,
    pub scope: ScopeResult,
    pub steps: Vec<TaskResult>,
    pub passed: bool,
}

impl VerificationReport {
    fn write(&self, project: &Project) -> Result<()> {
        fs::create_dir_all(&project.reports)?;
        fs::write(
            project.reports.join("test_result.json"),
            serde_json::to_string_pretty(self)?,
        )?;

        let mut markdown = String::from("=== Verification report ===\n");
        markdown.push_str(&format!("Timestamp: {}\n", self.timestamp));
        markdown.push_str(&format!("Profile: {}\n", self.profile));
        let components = self
            .scope
            .components
            .iter()
            .map(|component| component.label())
            .collect::<Vec<_>>()
            .join(",");
        markdown.push_str(&format!("Components: {components}\n\n"));
        for step in &self.steps {
            let status = if step.passed { "PASS" } else { "FAIL" };
            markdown.push_str(&format!(
                "- {status}: {} ({} ms)",
                step.label, step.duration_ms
            ));
            if let Some(detail) = &step.detail {
                markdown.push_str(&format!(" - {detail}"));
            }
            if !step.passed {
                markdown.push_str(&format!("; log: {}", step.log));
            }
            markdown.push('\n');
        }
        let summary = if self.passed { "PASS" } else { "FAIL" };
        markdown.push_str(&format!("\nTEST_SUMMARY: {summary}\n"));
        fs::write(project.reports.join("test_result.md"), markdown)?;
        Ok(())
    }
}

pub fn run(project: &Project, scope: ScopeResult, profile: Profile) -> Result<VerificationReport> {
    println!("arc-flow verify");
    let selected = scope
        .components
        .iter()
        .map(|component| component.label())
        .collect::<Vec<_>>()
        .join(", ");
    println!("Scope: {}", scope.mode);
    println!(
        "Components: {}\n",
        if selected.is_empty() {
            "none"
        } else {
            &selected
        }
    );

    scope.write_reports(project)?;
    let mut steps = Vec::new();
    let secret_mode = match profile {
        Profile::Full => SecretMode::WorkingTree,
        Profile::Hook => SecretMode::Staged,
    };
    let secret_started = Instant::now();
    let findings = secrets::scan(project, secret_mode)?;
    let secret_passed = findings.is_empty();
    let secret_result = TaskResult {
        label: "secret scan".to_string(),
        passed: secret_passed,
        timed_out: false,
        cancelled: false,
        duration_ms: secret_started.elapsed().as_millis(),
        log: project
            .reports
            .join("secret_scan.json")
            .to_string_lossy()
            .to_string(),
        detail: (!secret_passed).then(|| format!("{} file(s) require review", findings.len())),
    };
    print_result(&secret_result);
    steps.push(secret_result);

    if secret_passed {
        let audit_started = Instant::now();
        let outcome = audit::run(&project.audit_config, &project.reports, false)?;
        let audit_passed = outcome.total_violations == 0;
        let audit_result = TaskResult {
            label: "architecture audit".to_string(),
            passed: audit_passed,
            timed_out: false,
            cancelled: false,
            duration_ms: audit_started.elapsed().as_millis(),
            log: project
                .reports
                .join("review_context.json")
                .to_string_lossy()
                .to_string(),
            detail: Some(format!(
                "{} violation(s), {} blocker(s), {} error(s)",
                outcome.total_violations, outcome.blocker_count, outcome.error_count
            )),
        };
        print_result(&audit_result);
        steps.push(audit_result);
    }

    if crate::process::cancelled() {
        bail!("verification cancelled");
    }
    if steps.iter().all(|step| step.passed) {
        if scope.components.contains(&Component::Backend) {
            run_backend(project, profile, &mut steps)?;
        }
        if scope.components.contains(&Component::Frontend) {
            run_frontend(project, profile, &mut steps)?;
        }
        if scope.components.contains(&Component::Workflow) {
            run_workflow(project, profile, &mut steps)?;
        }
    }

    let passed = steps.iter().all(|step| step.passed);
    let report = VerificationReport {
        timestamp: chrono::Utc::now().to_rfc3339(),
        profile: match profile {
            Profile::Full => "full",
            Profile::Hook => "hook",
        }
        .to_string(),
        scope,
        steps,
        passed,
    };
    report.write(project)?;
    println!(
        "\nVerification report: {}",
        project.reports.join("test_result.md").display()
    );
    println!(
        "TEST_SUMMARY: {}",
        if report.passed { "PASS" } else { "FAIL" }
    );
    Ok(report)
}

fn run_backend(project: &Project, profile: Profile, steps: &mut Vec<TaskResult>) -> Result<()> {
    let manifest = project.backend.join("Cargo.toml");
    execute(
        Task::new(
            "backend format",
            "cargo",
            &project.root,
            log(project, "backend_fmt.log"),
        )
        .args([
            "fmt",
            "--manifest-path",
            &manifest.to_string_lossy(),
            "--",
            "--check",
        ]),
        None,
        steps,
    )?;
    execute(
        Task::new(
            "backend Clippy",
            "cargo",
            &project.root,
            log(project, "backend_clippy.log"),
        )
        .args([
            "clippy",
            "--manifest-path",
            &manifest.to_string_lossy(),
            "--locked",
            "--all-targets",
            "--all-features",
            "--",
            "-D",
            "warnings",
        ]),
        None,
        steps,
    )?;

    if matches!(profile, Profile::Hook) {
        return Ok(());
    }

    execute(
        Task::new(
            "backend compile",
            "cargo",
            &project.root,
            log(project, "backend_check.log"),
        )
        .args([
            "check",
            "--manifest-path",
            &manifest.to_string_lossy(),
            "--locked",
            "--all-targets",
        ]),
        None,
        steps,
    )?;

    let database = match TestDatabase::prepare(project) {
        Ok(database) => database,
        Err(error) => {
            let result = TaskResult {
                label: "test database setup".to_string(),
                passed: false,
                timed_out: false,
                cancelled: false,
                duration_ms: 0,
                log: String::new(),
                detail: Some(format!("{error:#}")),
            };
            print_result(&result);
            steps.push(result);
            return Ok(());
        }
    };
    let timeout = env_seconds("RUST_TEST_TIMEOUT", 120);
    execute(
        Task::new(
            "backend tests",
            "cargo",
            &project.root,
            log(project, "backend_tests.log"),
        )
        .args([
            "test",
            "--manifest-path",
            &manifest.to_string_lossy(),
            "--locked",
            "--",
            "--nocapture",
        ])
        .env("TEST_DATABASE_URL", &database.url)
        .timeout(timeout),
        Some(TestCount::Rust),
        steps,
    )?;
    Ok(())
}

fn run_frontend(project: &Project, profile: Profile, steps: &mut Vec<TaskResult>) -> Result<()> {
    execute(
        Task::new(
            "frontend lint",
            "npm",
            &project.frontend,
            log(project, "frontend_lint.log"),
        )
        .args([
            "exec",
            "--offline",
            "eslint",
            "--",
            "src",
            "--max-warnings=0",
        ]),
        None,
        steps,
    )?;
    execute(
        Task::new(
            "frontend format",
            "npm",
            &project.frontend,
            log(project, "frontend_format.log"),
        )
        .args(["run", "format:check"]),
        None,
        steps,
    )?;

    if matches!(profile, Profile::Hook) {
        return Ok(());
    }

    execute(
        Task::new(
            "frontend tests",
            "npm",
            &project.frontend,
            log(project, "frontend_tests.log"),
        )
        .args(["test", "--", "--watch=false", "--runner=vitest"])
        .timeout(env_seconds("ANGULAR_TEST_TIMEOUT", 180)),
        Some(TestCount::Angular),
        steps,
    )?;
    execute(
        Task::new(
            "frontend production build",
            "npm",
            &project.frontend,
            log(project, "frontend_build.log"),
        )
        .args(["run", "build"])
        .timeout(env_seconds("ANGULAR_BUILD_TIMEOUT", 180)),
        None,
        steps,
    )?;
    Ok(())
}

fn run_workflow(project: &Project, profile: Profile, steps: &mut Vec<TaskResult>) -> Result<()> {
    let manifest = project.tool_manifest.to_string_lossy();
    execute(
        Task::new(
            "Git hook syntax",
            "sh",
            &project.root,
            log(project, "git_hook_syntax.log"),
        )
        .args([
            "-n",
            &project
                .root
                .join("codex-audit-pipeline/hooks/pre-commit")
                .to_string_lossy(),
        ]),
        None,
        steps,
    )?;
    execute(
        Task::new(
            "arc-flow format",
            "cargo",
            &project.root,
            log(project, "arc_flow_fmt.log"),
        )
        .args(["fmt", "--manifest-path", &manifest, "--", "--check"]),
        None,
        steps,
    )?;
    if matches!(profile, Profile::Full) {
        execute(
            Task::new(
                "arc-flow Clippy",
                "cargo",
                &project.root,
                log(project, "arc_flow_clippy.log"),
            )
            .args([
                "clippy",
                "--manifest-path",
                &manifest,
                "--locked",
                "--all-targets",
                "--all-features",
                "--",
                "-D",
                "warnings",
            ]),
            None,
            steps,
        )?;
    }
    execute(
        Task::new(
            "arc-flow tests",
            "cargo",
            &project.root,
            log(project, "arc_flow_tests.log"),
        )
        .args([
            "test",
            "--manifest-path",
            &manifest,
            "--locked",
            "--",
            "--nocapture",
        ])
        .timeout(env_seconds("RUST_TEST_TIMEOUT", 120)),
        Some(TestCount::Rust),
        steps,
    )?;
    Ok(())
}

#[derive(Clone, Copy)]
enum TestCount {
    Rust,
    Angular,
}

fn execute(task: Task, count: Option<TestCount>, steps: &mut Vec<TaskResult>) -> Result<()> {
    print!("[RUN ] {} ... ", task.label);
    use std::io::Write;
    std::io::stdout().flush().ok();
    let mut result = task.run()?;
    if result.passed {
        if let Some(kind) = count {
            let content = fs::read_to_string(&result.log).unwrap_or_default();
            let tests = count_tests(&content, kind)?;
            if tests == 0 {
                result.passed = false;
                result.detail = Some("executed 0 tests".to_string());
            } else {
                result.detail = Some(format!("{tests} test(s)"));
            }
        }
    }
    print_result_inline(&result);
    if result.cancelled {
        bail!("verification cancelled");
    }
    steps.push(result);
    Ok(())
}

fn count_tests(content: &str, kind: TestCount) -> Result<usize> {
    let pattern = match kind {
        TestCount::Rust => Regex::new(r"(?m)^running ([0-9]+) tests?$")?,
        TestCount::Angular => Regex::new(r"Tests\s+([0-9]+) passed")?,
    };
    Ok(pattern
        .captures_iter(content)
        .filter_map(|capture| capture[1].parse::<usize>().ok())
        .sum())
}

fn print_result(result: &TaskResult) {
    let marker = if result.passed { "PASS" } else { "FAIL" };
    println!("[{marker}] {} ({} ms)", result.label, result.duration_ms);
    if !result.passed {
        println!("       log: {}", result.log);
    }
}

fn print_result_inline(result: &TaskResult) {
    let marker = if result.passed { "PASS" } else { "FAIL" };
    println!("{marker} ({} ms)", result.duration_ms);
    if !result.passed {
        println!("       log: {}", result.log);
    }
}

fn log(project: &Project, name: &str) -> std::path::PathBuf {
    project.reports.join("logs").join(name)
}

fn env_seconds(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

struct TestDatabase {
    url: String,
    container: Option<String>,
}

impl TestDatabase {
    fn prepare(project: &Project) -> Result<Self> {
        if let Ok(url) = std::env::var("TEST_DATABASE_URL") {
            return Ok(Self {
                url,
                container: None,
            });
        }

        let info = Command::new("docker")
            .current_dir(&project.root)
            .arg("info")
            .output()
            .context("Docker is required when TEST_DATABASE_URL is not set")?;
        if !info.status.success() {
            bail!("Docker daemon is unavailable; set TEST_DATABASE_URL or grant daemon access");
        }

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let name = format!("arc-admin-test-{}-{unique}", std::process::id());
        let output = Command::new("docker")
            .args([
                "run",
                "--rm",
                "--detach",
                "--pull=never",
                "--name",
                &name,
                "--env",
                "POSTGRES_USER=arc_admin_test",
                "--env",
                "POSTGRES_PASSWORD=arc_admin_test",
                "--env",
                "POSTGRES_DB=arc_admin_test",
                "--publish",
                "127.0.0.1::5432",
                POSTGRES_IMAGE,
            ])
            .output()
            .context("start temporary PostgreSQL")?;
        if !output.status.success() {
            bail!(
                "failed to start temporary PostgreSQL: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }

        let mut database = Self {
            url: String::new(),
            container: Some(name),
        };
        for _ in 0..30 {
            if crate::process::cancelled() {
                bail!("verification cancelled while waiting for PostgreSQL");
            }
            if let Some(port) = database.port()? {
                let ready = Command::new("docker")
                    .args([
                        "exec",
                        database.container.as_deref().unwrap_or_default(),
                        "pg_isready",
                        "-U",
                        "arc_admin_test",
                        "-d",
                        "arc_admin_test",
                    ])
                    .output()
                    .is_ok_and(|value| value.status.success());
                if ready {
                    database.url = format!(
                        "postgres://arc_admin_test:arc_admin_test@127.0.0.1:{port}/arc_admin_test"
                    );
                    return Ok(database);
                }
            }
            thread::sleep(Duration::from_secs(1));
        }
        bail!("temporary PostgreSQL did not become ready within 30 seconds")
    }

    fn port(&self) -> Result<Option<String>> {
        let Some(container) = &self.container else {
            return Ok(None);
        };
        let output = Command::new("docker")
            .args(["port", container, "5432/tcp"])
            .output()?;
        if !output.status.success() {
            return Ok(None);
        }
        Ok(String::from_utf8_lossy(&output.stdout)
            .lines()
            .next()
            .and_then(|line| line.rsplit(':').next())
            .map(str::to_string))
    }
}

impl Drop for TestDatabase {
    fn drop(&mut self) {
        if let Some(container) = &self.container {
            let _ = Command::new("docker")
                .args(["rm", "--force", container])
                .output();
        }
    }
}

pub fn explicit_scope(components: &[Component]) -> ScopeResult {
    ScopeResult {
        mode: "components".to_string(),
        changed_files: Vec::new(),
        components: components.iter().copied().collect::<BTreeSet<_>>(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_rust_tests_across_multiple_binaries() {
        let log = "running 3 tests\n...\nrunning 2 tests\n";
        assert_eq!(count_tests(log, TestCount::Rust).expect("count"), 5);
    }

    #[test]
    fn counts_angular_tests() {
        let log = "Tests  19 passed (19)";
        assert_eq!(count_tests(log, TestCount::Angular).expect("count"), 19);
    }
}
