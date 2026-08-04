use crate::project::Project;
use anyhow::{bail, Context, Result};
use regex::bytes::Regex;
use serde::Serialize;
use std::fs;
use std::process::Command;

const SECRET_PATTERN: &str = r"github_pat_[A-Za-z0-9_]{20,}|gh[pousr]_[A-Za-z0-9]{20,}|glpat-[A-Za-z0-9_-]{20,}|AKIA[0-9A-Z]{16}|npm_[A-Za-z0-9]{36}|https?://[^/@\s]+:[^@\s]+@[^\s]+|-----BEGIN ([A-Z0-9 ]+ )?PRIVATE KEY-----";

#[derive(Debug, Clone, Copy)]
pub enum SecretMode {
    WorkingTree,
    Staged,
}

#[derive(Debug, Serialize)]
struct SecretReport<'a> {
    timestamp: String,
    mode: &'a str,
    findings: &'a [String],
}

pub fn scan(project: &Project, mode: SecretMode) -> Result<Vec<String>> {
    let files = match mode {
        SecretMode::WorkingTree => git_files(
            project,
            &[
                "ls-files",
                "--cached",
                "--others",
                "--exclude-standard",
                "-z",
            ],
        )?,
        SecretMode::Staged => git_files(
            project,
            &[
                "diff",
                "--cached",
                "--diff-filter=ACMR",
                "--name-only",
                "-z",
            ],
        )?,
    };
    let pattern = Regex::new(SECRET_PATTERN).context("compile secret pattern")?;
    let mut findings = Vec::new();

    for file in files {
        let bytes = match mode {
            SecretMode::WorkingTree => match fs::read(project.root.join(&file)) {
                Ok(bytes) => bytes,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error).with_context(|| format!("read {file}")),
            },
            SecretMode::Staged => {
                let output = Command::new("git")
                    .current_dir(&project.root)
                    .args(["show", &format!(":{file}")])
                    .output()
                    .with_context(|| format!("read staged file {file}"))?;
                if !output.status.success() {
                    continue;
                }
                output.stdout
            }
        };
        if pattern.is_match(&bytes) {
            findings.push(file);
        }
    }

    let mode_label = match mode {
        SecretMode::WorkingTree => "working-tree",
        SecretMode::Staged => "staged",
    };
    fs::create_dir_all(&project.reports)?;
    fs::write(
        project.reports.join("secret_scan.json"),
        serde_json::to_string_pretty(&SecretReport {
            timestamp: chrono::Utc::now().to_rfc3339(),
            mode: mode_label,
            findings: &findings,
        })?,
    )?;
    Ok(findings)
}

fn git_files(project: &Project, args: &[&str]) -> Result<Vec<String>> {
    let output = Command::new("git")
        .current_dir(&project.root)
        .args(args)
        .output()
        .with_context(|| format!("run git {}", args.join(" ")))?;
    if !output.status.success() {
        bail!("git {} failed", args.join(" "));
    }
    output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty())
        .map(|entry| String::from_utf8(entry.to_vec()).context("non-UTF-8 Git path"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_high_confidence_tokens_without_matching_placeholders() {
        let pattern = Regex::new(SECRET_PATTERN).expect("secret regex");
        let github_token = ["token=gh", "p_abcdefghijklmnopqrstuvwxyz123456"].concat();
        let access_key = ["AK", "IAIOSFODNN7EXAMPLE"].concat();
        assert!(pattern.is_match(github_token.as_bytes()));
        assert!(pattern.is_match(access_key.as_bytes()));
        assert!(!pattern.is_match(b"JWT_SECRET=change-me-in-production"));
    }
}
