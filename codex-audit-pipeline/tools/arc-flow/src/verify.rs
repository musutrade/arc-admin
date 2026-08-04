use crate::audit;
use crate::config::{Profile, StepConfig, TestParser};
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
    run_selected(project, scope, profile, None)
}

pub fn run_step(project: &Project, id: &str) -> Result<VerificationReport> {
    let step = project
        .config
        .step(id)
        .ok_or_else(|| anyhow::anyhow!("unknown verification step {id:?}"))?;
    let scope = explicit_scope(&[step.component]);
    run_selected(project, scope, Profile::Full, Some(id))
}

fn run_selected(
    project: &Project,
    scope: ScopeResult,
    profile: Profile,
    only_step: Option<&str>,
) -> Result<VerificationReport> {
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
        run_configured_steps(project, &scope, profile, only_step, &mut steps)?;
    }

    let passed = steps.iter().all(|step| step.passed);
    let report = VerificationReport {
        timestamp: chrono::Utc::now().to_rfc3339(),
        profile: only_step
            .map(|id| format!("step:{id}"))
            .unwrap_or_else(|| profile.label().to_string()),
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

fn run_configured_steps(
    project: &Project,
    scope: &ScopeResult,
    profile: Profile,
    only_step: Option<&str>,
    steps: &mut Vec<TaskResult>,
) -> Result<()> {
    let selected = project.config.steps.iter().filter(|step| {
        scope.components.contains(&step.component)
            && only_step
                .map(|id| step.id == id)
                .unwrap_or_else(|| step.profiles.contains(&profile))
    });
    let mut database = None;

    for step in selected {
        if crate::process::cancelled() {
            bail!("verification cancelled");
        }
        if step.requires_test_database && database.is_none() {
            match TestDatabase::prepare(project) {
                Ok(value) => database = Some(value),
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
                    break;
                }
            }
        }
        let database_url = database.as_ref().map(|value| value.url.as_str());
        execute(
            configured_task(project, step, database_url),
            step.parser,
            steps,
        )?;
    }
    Ok(())
}

fn configured_task(project: &Project, step: &StepConfig, database_url: Option<&str>) -> Task {
    let cwd = std::path::PathBuf::from(project.expand(&step.cwd));
    let args = step
        .args
        .iter()
        .map(|argument| project.expand(argument))
        .collect::<Vec<_>>();
    let mut task = Task::new(&step.label, &step.program, &cwd, log(project, &step.log))
        .args(args)
        .timeout(step.timeout_secs);
    if step.requires_test_database {
        task = task.env(
            "TEST_DATABASE_URL",
            database_url.expect("validated database step has a prepared database"),
        );
    }
    task
}

fn execute(task: Task, count: Option<TestParser>, steps: &mut Vec<TaskResult>) -> Result<()> {
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

fn count_tests(content: &str, kind: TestParser) -> Result<usize> {
    let pattern = match kind {
        TestParser::Rust => Regex::new(r"(?m)^running ([0-9]+) tests?$")?,
        TestParser::Angular => Regex::new(r"Tests\s+([0-9]+) passed")?,
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
        let config = &project.config.database;
        let user = format!("POSTGRES_USER={}", config.user);
        let password = format!("POSTGRES_PASSWORD={}", config.password);
        let database_name = format!("POSTGRES_DB={}", config.name);
        let publish = format!("127.0.0.1::{}", config.container_port);
        let output = Command::new("docker")
            .args([
                "run",
                "--rm",
                "--detach",
                "--pull=never",
                "--name",
                &name,
                "--env",
                &user,
                "--env",
                &password,
                "--env",
                &database_name,
                "--publish",
                &publish,
                &config.image,
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
        let deadline = Instant::now() + Duration::from_secs(config.startup_timeout_secs);
        while Instant::now() < deadline {
            if crate::process::cancelled() {
                bail!("verification cancelled while waiting for PostgreSQL");
            }
            if let Some(port) = database.port(config.container_port)? {
                let ready = Command::new("docker")
                    .args([
                        "exec",
                        database.container.as_deref().unwrap_or_default(),
                        "pg_isready",
                        "-U",
                        &config.user,
                        "-d",
                        &config.name,
                    ])
                    .output()
                    .is_ok_and(|value| value.status.success());
                if ready {
                    database.url = format!(
                        "postgres://{}:{}@127.0.0.1:{port}/{}",
                        config.user, config.password, config.name
                    );
                    return Ok(database);
                }
            }
            thread::sleep(Duration::from_secs(1));
        }
        bail!(
            "temporary PostgreSQL did not become ready within {} seconds",
            config.startup_timeout_secs
        )
    }

    fn port(&self, container_port: u16) -> Result<Option<String>> {
        let Some(container) = &self.container else {
            return Ok(None);
        };
        let output = Command::new("docker")
            .args(["port", container, &format!("{container_port}/tcp")])
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
        assert_eq!(count_tests(log, TestParser::Rust).expect("count"), 5);
    }

    #[test]
    fn counts_angular_tests() {
        let log = "Tests  19 passed (19)";
        assert_eq!(count_tests(log, TestParser::Angular).expect("count"), 19);
    }
}
