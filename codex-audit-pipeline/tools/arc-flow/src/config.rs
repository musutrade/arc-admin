use crate::scope::Component;
use anyhow::{bail, Context, Result};
use clap::ValueEnum;
use globset::{Glob, GlobSetBuilder};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashSet};
use std::env;
use std::fs;
use std::path::{Component as PathComponent, Path, PathBuf};

pub const CONFIG_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Profile {
    Full,
    Hook,
}

impl Profile {
    pub fn label(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Hook => "hook",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlowConfig {
    pub version: u32,
    pub paths: PathsConfig,
    pub doctor: DoctorConfig,
    pub database: DatabaseConfig,
    pub scope: ScopeConfig,
    pub steps: Vec<StepConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PathsConfig {
    pub backend: String,
    pub frontend: String,
    pub reports: String,
    pub tool_manifest: String,
    pub audit_config: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DoctorConfig {
    pub required_commands: Vec<String>,
    pub node_version_file: String,
    pub hooks_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DatabaseConfig {
    pub image: String,
    pub startup_timeout_secs: u64,
    pub container_port: u16,
    pub user: String,
    pub password: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScopeConfig {
    pub rules: Vec<ScopeRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScopeRule {
    pub patterns: Vec<String>,
    pub components: BTreeSet<Component>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TestParser {
    Rust,
    Angular,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StepConfig {
    pub id: String,
    pub label: String,
    pub component: Component,
    pub profiles: BTreeSet<Profile>,
    pub program: String,
    pub args: Vec<String>,
    pub cwd: String,
    pub log: String,
    pub timeout_secs: u64,
    #[serde(default)]
    pub timeout_env: Option<String>,
    #[serde(default)]
    pub parser: Option<TestParser>,
    #[serde(default)]
    pub requires_test_database: bool,
}

impl FlowConfig {
    pub fn load(path: &Path) -> Result<Self> {
        let source = fs::read_to_string(path)
            .with_context(|| format!("read workflow config {}", path.display()))?;
        let mut config: Self = toml::from_str(&source)
            .with_context(|| format!("parse workflow config {}", path.display()))?;
        config.apply_environment()?;
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<()> {
        if self.version != CONFIG_VERSION {
            bail!(
                "unsupported workflow config version {}; expected {}",
                self.version,
                CONFIG_VERSION
            );
        }

        for (name, value) in [
            ("paths.backend", &self.paths.backend),
            ("paths.frontend", &self.paths.frontend),
            ("paths.reports", &self.paths.reports),
            ("paths.tool_manifest", &self.paths.tool_manifest),
            ("paths.audit_config", &self.paths.audit_config),
            ("doctor.node_version_file", &self.doctor.node_version_file),
            ("doctor.hooks_path", &self.doctor.hooks_path),
        ] {
            validate_repo_path(name, value)?;
        }

        if self.doctor.required_commands.is_empty() {
            bail!("doctor.required_commands must not be empty");
        }
        for command in &self.doctor.required_commands {
            validate_program("doctor.required_commands", command)?;
        }

        validate_database(&self.database)?;
        self.validate_scope()?;
        self.validate_steps()?;
        Ok(())
    }

    pub fn components_for(&self, paths: &[String]) -> Result<BTreeSet<Component>> {
        let mut components = BTreeSet::new();
        for rule in &self.scope.rules {
            let mut builder = GlobSetBuilder::new();
            for pattern in &rule.patterns {
                builder.add(Glob::new(pattern)?);
            }
            let matcher = builder.build()?;
            if paths.iter().any(|path| matcher.is_match(path)) {
                components.extend(rule.components.iter().copied());
            }
        }
        Ok(components)
    }

    pub fn step(&self, id: &str) -> Option<&StepConfig> {
        self.steps.iter().find(|step| step.id == id)
    }

    fn apply_environment(&mut self) -> Result<()> {
        override_string("ARC_FLOW_BACKEND", &mut self.paths.backend);
        override_string("ARC_FLOW_FRONTEND", &mut self.paths.frontend);
        override_string("REPORT_DIR", &mut self.paths.reports);
        override_string("ARC_FLOW_REPORTS", &mut self.paths.reports);
        override_string("ARC_FLOW_TOOL_MANIFEST", &mut self.paths.tool_manifest);
        override_string("AUDITOR_CONFIG", &mut self.paths.audit_config);
        override_string("ARC_FLOW_AUDIT_CONFIG", &mut self.paths.audit_config);
        override_string("ARC_FLOW_POSTGRES_IMAGE", &mut self.database.image);
        override_u64(
            "ARC_FLOW_DATABASE_TIMEOUT_SECS",
            &mut self.database.startup_timeout_secs,
        )?;

        for step in &mut self.steps {
            if let Some(name) = &step.timeout_env {
                override_u64(name, &mut step.timeout_secs)?;
            }
        }
        Ok(())
    }

    fn validate_scope(&self) -> Result<()> {
        if self.scope.rules.is_empty() {
            bail!("scope.rules must not be empty");
        }
        for (index, rule) in self.scope.rules.iter().enumerate() {
            if rule.patterns.is_empty() || rule.components.is_empty() {
                bail!("scope.rules[{index}] requires patterns and components");
            }
            for pattern in &rule.patterns {
                if pattern.contains("..") || Path::new(pattern).is_absolute() {
                    bail!("scope.rules[{index}] contains unsafe pattern {pattern:?}");
                }
                Glob::new(pattern).with_context(|| {
                    format!("scope.rules[{index}] contains invalid pattern {pattern:?}")
                })?;
            }
        }
        Ok(())
    }

    fn validate_steps(&self) -> Result<()> {
        let mut ids = HashSet::new();
        for step in &self.steps {
            if !ids.insert(step.id.as_str()) {
                bail!("duplicate verification step id {:?}", step.id);
            }
            validate_step(step)?;
        }

        for required in REQUIRED_STEPS {
            let step = self.step(required.id).ok_or_else(|| {
                anyhow::anyhow!("required verification step {:?} is missing", required.id)
            })?;
            if step.component != required.component {
                bail!(
                    "step {:?} must belong to {}",
                    step.id,
                    required.component.label()
                );
            }
            for profile in required.profiles {
                if !step.profiles.contains(profile) {
                    bail!(
                        "step {:?} must run in the {} profile",
                        step.id,
                        profile.label()
                    );
                }
            }
            if step.requires_test_database != required.database {
                bail!(
                    "step {:?} requires_test_database must be {}",
                    step.id,
                    required.database
                );
            }
            if step.parser != required.parser {
                bail!("step {:?} must use its protected test parser", step.id);
            }
        }
        Ok(())
    }
}

fn validate_database(database: &DatabaseConfig) -> Result<()> {
    if database.image.trim().is_empty()
        || database.user.trim().is_empty()
        || database.password.is_empty()
        || database.name.trim().is_empty()
    {
        bail!("database image, user, password, and name must not be empty");
    }
    if database.image.starts_with('-')
        || !database.image.chars().all(|character| {
            character.is_ascii_alphanumeric()
                || matches!(character, '.' | '_' | '/' | ':' | '@' | '-')
        })
    {
        bail!("database.image must be an OCI image reference, not a Docker option");
    }
    if database.startup_timeout_secs == 0 || database.startup_timeout_secs > 300 {
        bail!("database.startup_timeout_secs must be between 1 and 300");
    }
    if database.container_port == 0 {
        bail!("database.container_port must not be zero");
    }
    for (name, value) in [
        ("database.user", &database.user),
        ("database.password", &database.password),
        ("database.name", &database.name),
    ] {
        if !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
        {
            bail!("{name} may only contain ASCII letters, digits, and underscores");
        }
    }
    Ok(())
}

fn validate_step(step: &StepConfig) -> Result<()> {
    if step.id.is_empty()
        || !step.id.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '.' | '-' | '_')
        })
    {
        bail!("invalid verification step id {:?}", step.id);
    }
    if step.label.trim().is_empty() || step.profiles.is_empty() {
        bail!(
            "step {:?} requires a label and at least one profile",
            step.id
        );
    }
    validate_program(&format!("step {} program", step.id), &step.program)?;
    if matches!(step.program.as_str(), "sh" | "bash" | "dash" | "zsh")
        && step.args.iter().any(|arg| {
            arg.starts_with("--command")
                || (arg.starts_with('-') && !arg.starts_with("--") && arg[1..].contains('c'))
        })
    {
        bail!("step {:?} may not execute a shell command string", step.id);
    }
    if !matches!(step.cwd.as_str(), "{root}" | "{backend}" | "{frontend}") {
        bail!("step {:?} has unsupported cwd {:?}", step.id, step.cwd);
    }
    let log = Path::new(&step.log);
    if log.components().count() != 1 || log.extension().is_none_or(|value| value != "log") {
        bail!("step {:?} log must be a single .log file name", step.id);
    }
    if step.timeout_secs == 0 || step.timeout_secs > 3600 {
        bail!("step {:?} timeout_secs must be between 1 and 3600", step.id);
    }
    if let Some(name) = &step.timeout_env {
        if name.is_empty()
            || !name.chars().all(|character| {
                character.is_ascii_uppercase() || character.is_ascii_digit() || character == '_'
            })
        {
            bail!("step {:?} has invalid timeout_env {:?}", step.id, name);
        }
    }
    for arg in &step.args {
        validate_argument(&step.id, arg)?;
    }
    if step.requires_test_database && step.component != Component::Backend {
        bail!("only backend steps may request the test database");
    }
    Ok(())
}

fn validate_argument(step_id: &str, value: &str) -> Result<()> {
    if value.contains('\0') {
        bail!("step {step_id:?} contains a NUL command argument");
    }
    let mut rest = value;
    while let Some(start) = rest.find('{') {
        let tail = &rest[start..];
        let Some(end) = tail.find('}') else {
            bail!("step {step_id:?} contains an unterminated placeholder in {value:?}");
        };
        let placeholder = &tail[..=end];
        if !matches!(
            placeholder,
            "{root}"
                | "{backend}"
                | "{frontend}"
                | "{reports}"
                | "{tool_manifest}"
                | "{audit_config}"
        ) {
            bail!("step {step_id:?} contains unsupported placeholder {placeholder:?}");
        }
        rest = &tail[end + 1..];
    }
    Ok(())
}

fn validate_program(name: &str, value: &str) -> Result<()> {
    if value.is_empty()
        || value.contains('/')
        || value.contains('\\')
        || !value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.' | '+')
        })
    {
        bail!("{name} must be a bare executable name, found {value:?}");
    }
    Ok(())
}

fn validate_repo_path(name: &str, value: &str) -> Result<()> {
    let path = Path::new(value);
    if value.is_empty() || path.is_absolute() {
        bail!("{name} must be a non-empty repository-relative path");
    }
    if path.components().any(|component| {
        matches!(
            component,
            PathComponent::ParentDir | PathComponent::RootDir | PathComponent::Prefix(_)
        )
    }) {
        bail!("{name} may not escape the repository: {value:?}");
    }
    Ok(())
}

fn override_string(name: &str, target: &mut String) {
    if let Ok(value) = env::var(name) {
        *target = value;
    }
}

fn override_u64(name: &str, target: &mut u64) -> Result<()> {
    if let Ok(value) = env::var(name) {
        *target = value
            .parse()
            .with_context(|| format!("environment variable {name} must be an integer"))?;
    }
    Ok(())
}

struct RequiredStep {
    id: &'static str,
    component: Component,
    profiles: &'static [Profile],
    database: bool,
    parser: Option<TestParser>,
}

const FULL: &[Profile] = &[Profile::Full];
const FULL_AND_HOOK: &[Profile] = &[Profile::Full, Profile::Hook];
const REQUIRED_STEPS: &[RequiredStep] = &[
    RequiredStep {
        id: "backend.format",
        component: Component::Backend,
        profiles: FULL_AND_HOOK,
        database: false,
        parser: None,
    },
    RequiredStep {
        id: "backend.clippy",
        component: Component::Backend,
        profiles: FULL_AND_HOOK,
        database: false,
        parser: None,
    },
    RequiredStep {
        id: "backend.compile",
        component: Component::Backend,
        profiles: FULL,
        database: false,
        parser: None,
    },
    RequiredStep {
        id: "backend.tests",
        component: Component::Backend,
        profiles: FULL,
        database: true,
        parser: Some(TestParser::Rust),
    },
    RequiredStep {
        id: "frontend.lint",
        component: Component::Frontend,
        profiles: FULL_AND_HOOK,
        database: false,
        parser: None,
    },
    RequiredStep {
        id: "frontend.format",
        component: Component::Frontend,
        profiles: FULL_AND_HOOK,
        database: false,
        parser: None,
    },
    RequiredStep {
        id: "frontend.tests",
        component: Component::Frontend,
        profiles: FULL,
        database: false,
        parser: Some(TestParser::Angular),
    },
    RequiredStep {
        id: "frontend.build",
        component: Component::Frontend,
        profiles: FULL,
        database: false,
        parser: None,
    },
    RequiredStep {
        id: "workflow.hook-syntax",
        component: Component::Workflow,
        profiles: FULL_AND_HOOK,
        database: false,
        parser: None,
    },
    RequiredStep {
        id: "workflow.format",
        component: Component::Workflow,
        profiles: FULL_AND_HOOK,
        database: false,
        parser: None,
    },
    RequiredStep {
        id: "workflow.clippy",
        component: Component::Workflow,
        profiles: FULL,
        database: false,
        parser: None,
    },
    RequiredStep {
        id: "workflow.tests",
        component: Component::Workflow,
        profiles: FULL_AND_HOOK,
        database: false,
        parser: Some(TestParser::Rust),
    },
];

pub fn resolve_config_path(root: &Path, override_path: Option<PathBuf>) -> Result<PathBuf> {
    let path = override_path
        .or_else(|| env::var_os("ARC_FLOW_CONFIG").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("codex-audit-pipeline/.codex/flow.toml"));
    let path = if path.is_absolute() {
        path
    } else {
        root.join(path)
    };
    let path = path
        .canonicalize()
        .with_context(|| format!("resolve workflow config {}", path.display()))?;
    if !path.starts_with(root) {
        bail!(
            "workflow config must be inside the repository: {}",
            path.display()
        );
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repository_config() -> FlowConfig {
        toml::from_str(include_str!("../../../.codex/flow.toml")).expect("parse fixture")
    }

    #[test]
    fn repository_configuration_is_valid() {
        repository_config().validate().expect("validate config");
    }

    #[test]
    fn required_steps_cannot_be_removed() {
        let mut config = repository_config();
        config.steps.retain(|step| step.id != "backend.tests");
        let error = config.validate().expect_err("missing step must fail");
        assert!(error.to_string().contains("backend.tests"));
    }

    #[test]
    fn shell_command_strings_are_rejected() {
        let mut config = repository_config();
        let step = config.steps.first_mut().expect("step");
        step.program = "sh".into();
        step.args = vec!["-lc".into(), "cargo fmt".into()];
        let error = config.validate().expect_err("shell command must fail");
        assert!(error.to_string().contains("shell command string"));
    }

    #[test]
    fn unknown_fields_are_rejected() {
        let source = include_str!("../../../.codex/flow.toml").replacen(
            "version = 1",
            "version = 1\nunknown = true",
            1,
        );
        assert!(toml::from_str::<FlowConfig>(&source).is_err());
    }
}
