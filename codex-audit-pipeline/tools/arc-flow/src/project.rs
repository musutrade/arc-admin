use anyhow::{bail, Context, Result};
use std::env;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Project {
    pub root: PathBuf,
    pub reports: PathBuf,
    pub backend: PathBuf,
    pub frontend: PathBuf,
    pub tool_manifest: PathBuf,
    pub audit_config: PathBuf,
}

impl Project {
    pub fn discover(override_root: Option<PathBuf>) -> Result<Self> {
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
                    "could not find project root above {}; expected frontend/, backend/, and codex-audit-pipeline/",
                    start.display()
                )
            })?;

        let pipeline = root.join("codex-audit-pipeline");
        let reports = env::var_os("REPORT_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| pipeline.join(".codex/reports"));
        let audit_config = env::var_os("AUDITOR_CONFIG")
            .map(PathBuf::from)
            .unwrap_or_else(|| pipeline.join(".codex/audit.toml"));

        let project = Self {
            backend: root.join("backend"),
            frontend: root.join("frontend"),
            tool_manifest: pipeline.join("tools/arc-flow/Cargo.toml"),
            audit_config,
            reports,
            root,
        };
        project.validate()?;
        Ok(project)
    }

    fn is_root(path: &Path) -> bool {
        path.join("frontend").is_dir()
            && path.join("backend").is_dir()
            && path
                .join("codex-audit-pipeline/.codex/audit.toml")
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
}
