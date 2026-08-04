use crate::project::Project;
use anyhow::{bail, Context, Result};
use clap::ValueEnum;
use serde::Serialize;
use std::collections::BTreeSet;
use std::process::Command;

#[derive(Debug, Clone)]
pub enum ScopeMode {
    WorkingTree,
    Staged,
    Base(String),
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Component {
    Backend,
    Frontend,
    Workflow,
}

impl Component {
    pub fn label(self) -> &'static str {
        match self {
            Self::Backend => "backend",
            Self::Frontend => "frontend",
            Self::Workflow => "workflow",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ScopeResult {
    pub mode: String,
    pub changed_files: Vec<String>,
    pub components: BTreeSet<Component>,
}

impl ScopeResult {
    pub fn all() -> Self {
        Self {
            mode: "all".to_string(),
            changed_files: Vec::new(),
            components: BTreeSet::from([
                Component::Backend,
                Component::Frontend,
                Component::Workflow,
            ]),
        }
    }

    pub fn write_reports(&self, project: &Project) -> Result<()> {
        std::fs::create_dir_all(&project.reports)?;
        let changed = if self.changed_files.is_empty() {
            String::new()
        } else {
            format!("{}\n", self.changed_files.join("\n"))
        };
        std::fs::write(project.reports.join("changed_files.txt"), changed)?;
        std::fs::write(
            project.reports.join("scope.json"),
            serde_json::to_string_pretty(self)?,
        )?;
        Ok(())
    }
}

pub fn detect(project: &Project, mode: &ScopeMode) -> Result<ScopeResult> {
    if matches!(mode, ScopeMode::All) {
        return Ok(ScopeResult::all());
    }

    ensure_git_worktree(project)?;
    let mut paths = BTreeSet::new();
    let mode_label = match mode {
        ScopeMode::WorkingTree => {
            paths.extend(git_paths(project, &["diff", "--name-only", "-z"])?);
            paths.extend(git_paths(
                project,
                &["diff", "--cached", "--name-only", "-z"],
            )?);
            paths.extend(git_paths(
                project,
                &["ls-files", "--others", "--exclude-standard", "-z"],
            )?);
            "working-tree".to_string()
        }
        ScopeMode::Staged => {
            paths.extend(git_paths(
                project,
                &["diff", "--cached", "--name-only", "-z"],
            )?);
            "staged".to_string()
        }
        ScopeMode::Base(reference) => {
            let output = Command::new("git")
                .current_dir(&project.root)
                .args(["rev-parse", "--verify", &format!("{reference}^{{commit}}")])
                .output()
                .context("run git rev-parse")?;
            if !output.status.success() {
                bail!("Git base reference does not exist: {reference}");
            }
            paths.extend(git_paths(
                project,
                &["diff", "--name-only", "-z", &format!("{reference}...HEAD")],
            )?);
            format!("base:{reference}")
        }
        ScopeMode::All => unreachable!(),
    };

    let changed_files = paths.into_iter().collect::<Vec<_>>();
    Ok(ScopeResult {
        mode: mode_label,
        components: classify(&changed_files),
        changed_files,
    })
}

fn ensure_git_worktree(project: &Project) -> Result<()> {
    let output = Command::new("git")
        .current_dir(&project.root)
        .args(["rev-parse", "--is-inside-work-tree"])
        .output()
        .context("run git rev-parse")?;
    if !output.status.success() || output.stdout != b"true\n" {
        bail!("project root is not a Git worktree");
    }
    Ok(())
}

fn git_paths(project: &Project, args: &[&str]) -> Result<Vec<String>> {
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
        .map(|entry| {
            String::from_utf8(entry.to_vec()).context("Git returned a non-UTF-8 file path")
        })
        .collect()
}

fn classify(paths: &[String]) -> BTreeSet<Component> {
    let mut components = BTreeSet::new();
    for path in paths {
        if path.starts_with("backend/") {
            components.insert(Component::Backend);
        }
        if path.starts_with("frontend/") || path == ".node-version" {
            components.insert(Component::Frontend);
        }
        if path == "rust-toolchain.toml" {
            components.insert(Component::Backend);
            components.insert(Component::Workflow);
        }
        if path.starts_with("codex-audit-pipeline/tools/arc-flow/")
            || path.starts_with("codex-audit-pipeline/hooks/")
            || path == ".cargo/config.toml"
        {
            components.extend([Component::Backend, Component::Frontend, Component::Workflow]);
        }
        if path == "docs/openapi.yaml" || path.starts_with(".github/workflows/") {
            components.extend([Component::Backend, Component::Frontend, Component::Workflow]);
        }
        if path.starts_with("codex-audit-pipeline/.codex/audit.toml")
            || path.starts_with("codex-audit-pipeline/.codex/templates/")
        {
            components.insert(Component::Workflow);
        }
    }
    components
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workflow_changes_force_all_components() {
        let components = classify(&["codex-audit-pipeline/tools/arc-flow/src/main.rs".into()]);
        assert_eq!(components.len(), 3);
    }

    #[test]
    fn frontend_change_only_selects_frontend() {
        let components = classify(&["frontend/src/main.ts".into()]);
        assert_eq!(components, BTreeSet::from([Component::Frontend]));
    }
}
