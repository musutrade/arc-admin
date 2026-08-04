use crate::config::{resolve_config_path, FlowConfig, DEFAULT_CONFIG_PATH};
use anyhow::{bail, Context, Result};
use std::collections::BTreeMap;
use std::env;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Project {
    pub root: PathBuf,
    pub config_path: PathBuf,
    pub config: FlowConfig,
    pub reports: PathBuf,
    pub audit_config: PathBuf,
    aliases: BTreeMap<String, PathBuf>,
}

impl Project {
    pub fn discover(
        override_root: Option<PathBuf>,
        config_override: Option<PathBuf>,
    ) -> Result<Self> {
        let root = match override_root.or_else(|| env::var_os("PROJECT_ROOT").map(PathBuf::from)) {
            Some(root) => canonical_directory(&root)?,
            None => {
                let start = env::current_dir().context("read current directory")?;
                find_root(&start, config_override.as_deref())?
            }
        };
        let config_path = resolve_config_path(&root, config_override)?;
        let config = FlowConfig::load(&config_path)?;
        let reports = root.join(&config.paths.reports);
        let audit_config = root.join(&config.paths.audit_config);
        let aliases = config
            .paths
            .aliases
            .iter()
            .map(|(name, entry)| (name.clone(), root.join(&entry.path)))
            .collect();

        let project = Self {
            root,
            config_path,
            config,
            reports,
            audit_config,
            aliases,
        };
        project.validate()?;
        Ok(project)
    }

    fn validate(&self) -> Result<()> {
        if !self.audit_config.is_file() {
            bail!(
                "required audit configuration is missing: {}",
                self.audit_config.display()
            );
        }
        Ok(())
    }

    pub fn prepare(&self) -> Result<()> {
        std::fs::create_dir_all(self.reports.join("logs"))?;
        env::set_current_dir(&self.root)
            .with_context(|| format!("enter project root {}", self.root.display()))?;
        Ok(())
    }

    pub fn path(&self, alias: &str) -> Option<&Path> {
        match alias {
            "root" => Some(&self.root),
            "reports" => Some(&self.reports),
            "audit_config" => Some(&self.audit_config),
            _ => self.aliases.get(alias).map(PathBuf::as_path),
        }
    }

    pub fn expand(&self, value: &str) -> String {
        let mut resolved = value.to_string();
        for name in self.config.paths.aliases.keys().map(String::as_str) {
            if let Some(path) = self.path(name) {
                resolved = resolved.replace(&format!("{{{name}}}"), &path.to_string_lossy());
            }
        }
        for name in ["audit_config", "reports", "root"] {
            if let Some(path) = self.path(name) {
                resolved = resolved.replace(&format!("{{{name}}}"), &path.to_string_lossy());
            }
        }
        resolved
    }
}

fn canonical_directory(path: &Path) -> Result<PathBuf> {
    let path = path
        .canonicalize()
        .with_context(|| format!("resolve project path {}", path.display()))?;
    if !path.is_dir() {
        bail!("project root is not a directory: {}", path.display());
    }
    Ok(path)
}

fn find_root(start: &Path, config_override: Option<&Path>) -> Result<PathBuf> {
    let start = canonical_directory(start)?;
    let configured_path = config_override
        .map(Path::to_path_buf)
        .or_else(|| env::var_os("ARC_FLOW_CONFIG").map(PathBuf::from));
    if let Some(config) = configured_path {
        let candidate = if config.is_absolute() {
            config
        } else {
            start.join(config)
        };
        let config = candidate
            .canonicalize()
            .with_context(|| format!("resolve workflow config {}", candidate.display()))?;
        if let Some(root) = config.ancestors().find(|path| path.join(".git").exists()) {
            return Ok(root.to_path_buf());
        }
    }
    start
        .ancestors()
        .find(|candidate| candidate.join(DEFAULT_CONFIG_PATH).is_file())
        .map(Path::to_path_buf)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "could not find project root above {}; expected {}; run `arc-flow init --preset <name>`",
                start.display(),
                DEFAULT_CONFIG_PATH
            )
        })
}
