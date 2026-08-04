use crate::config::{resolve_config_path, FlowConfig};
use anyhow::{bail, Context, Result};
use std::env;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Project {
    pub root: PathBuf,
    pub config_path: PathBuf,
    pub config: FlowConfig,
    pub reports: PathBuf,
    pub backend: PathBuf,
    pub frontend: PathBuf,
    pub tool_manifest: PathBuf,
    pub audit_config: PathBuf,
}

impl Project {
    pub fn discover(
        override_root: Option<PathBuf>,
        config_override: Option<PathBuf>,
    ) -> Result<Self> {
        let start = override_root
            .or_else(|| env::var_os("PROJECT_ROOT").map(PathBuf::from))
            .unwrap_or(env::current_dir().context("read current directory")?);
        let start = if start.is_file() {
            start.parent().unwrap_or(Path::new(".")).to_path_buf()
        } else {
            start
        };

        let root = start
            .canonicalize()
            .with_context(|| format!("resolve project path {}", start.display()))?
            .ancestors()
            .find(|candidate| Self::is_root(candidate))
            .map(Path::to_path_buf)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "could not find project root above {}; expected codex-audit-pipeline/.codex/ and tools/arc-flow/",
                    start.display()
                )
            })?;

        let config_path = resolve_config_path(&root, config_override)?;
        let config = FlowConfig::load(&config_path)?;

        let project = Self {
            backend: root.join(&config.paths.backend),
            frontend: root.join(&config.paths.frontend),
            tool_manifest: root.join(&config.paths.tool_manifest),
            audit_config: root.join(&config.paths.audit_config),
            reports: root.join(&config.paths.reports),
            config_path,
            config,
            root,
        };
        project.validate()?;
        Ok(project)
    }

    fn is_root(path: &Path) -> bool {
        path.join("codex-audit-pipeline/.codex").is_dir()
            && path
                .join("codex-audit-pipeline/tools/arc-flow/Cargo.toml")
                .is_file()
    }

    fn validate(&self) -> Result<()> {
        for required in [
            self.backend.join("Cargo.toml"),
            self.frontend.join("package.json"),
            self.tool_manifest.clone(),
            self.audit_config.clone(),
        ] {
            if !required.is_file() {
                bail!("required project file is missing: {}", required.display());
            }
        }
        Ok(())
    }

    pub fn prepare(&self) -> Result<()> {
        std::fs::create_dir_all(self.reports.join("logs"))?;
        env::set_current_dir(&self.root)
            .with_context(|| format!("enter project root {}", self.root.display()))?;
        Ok(())
    }

    pub fn expand(&self, value: &str) -> String {
        [
            ("{tool_manifest}", &self.tool_manifest),
            ("{audit_config}", &self.audit_config),
            ("{frontend}", &self.frontend),
            ("{backend}", &self.backend),
            ("{reports}", &self.reports),
            ("{root}", &self.root),
        ]
        .into_iter()
        .fold(value.to_string(), |resolved, (placeholder, path)| {
            resolved.replace(placeholder, &path.to_string_lossy())
        })
    }
}
