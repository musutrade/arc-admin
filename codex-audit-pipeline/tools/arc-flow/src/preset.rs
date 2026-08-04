use crate::config::{migrate_v1, FlowConfig, CONFIG_VERSION, DEFAULT_CONFIG_PATH};
use anyhow::{bail, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

struct Preset {
    name: &'static str,
    description: &'static str,
    flow: &'static str,
}

const AUDIT_TEMPLATE: &str = include_str!("../presets/empty.audit.toml");
const GITIGNORE_TEMPLATE: &str = "reports/\n";
const PRESETS: &[Preset] = &[
    Preset {
        name: "generic",
        description: "Git-based project with a minimal diff check",
        flow: include_str!("../presets/generic.flow.toml"),
    },
    Preset {
        name: "rust-api",
        description: "Single Rust crate with fmt, Clippy, check, and tests",
        flow: include_str!("../presets/rust-api.flow.toml"),
    },
    Preset {
        name: "angular-only",
        description: "Angular/npm project with lint, tests, and build",
        flow: include_str!("../presets/angular-only.flow.toml"),
    },
    Preset {
        name: "angular-rust-postgres",
        description: "Angular frontend, Rust backend, and temporary PostgreSQL",
        flow: include_str!("../presets/angular-rust-postgres.flow.toml"),
    },
];

pub fn print_presets() {
    for preset in PRESETS {
        println!("{:<24} {}", preset.name, preset.description);
    }
}

pub fn init(target: &Path, name: &str, force: bool) -> Result<()> {
    let preset = PRESETS
        .iter()
        .find(|preset| preset.name == name)
        .ok_or_else(|| anyhow::anyhow!("unknown preset {name:?}; run `arc-flow presets`"))?;
    fs::create_dir_all(target)
        .with_context(|| format!("create project directory {}", target.display()))?;
    let root = target
        .canonicalize()
        .with_context(|| format!("resolve project directory {}", target.display()))?;
    let flow_path = resolve_inside(&root, PathBuf::from(DEFAULT_CONFIG_PATH))?;
    let audit_path = resolve_inside(&root, PathBuf::from(".arc-flow/audit.toml"))?;
    ensure_writable(&flow_path, force)?;
    ensure_writable(&audit_path, force)?;

    let mut config = FlowConfig::from_source(preset.flow)?;
    config.project.name = project_id(&root);
    config.validate()?;
    let directory = flow_path.parent().context("flow config has no parent")?;
    fs::create_dir_all(directory)?;
    fs::write(&flow_path, toml::to_string_pretty(&config)?)?;
    fs::write(&audit_path, AUDIT_TEMPLATE)?;
    let gitignore = resolve_inside(&root, PathBuf::from(".arc-flow/.gitignore"))?;
    if !gitignore.exists() {
        fs::write(gitignore, GITIGNORE_TEMPLATE)?;
    }

    println!("Initialized preset {name:?} in {}", directory.display());
    println!(
        "Next: arc-flow --project-root {} config check",
        root.display()
    );
    Ok(())
}

pub fn migrate(
    root: &Path,
    input: Option<PathBuf>,
    output: Option<PathBuf>,
    force: bool,
) -> Result<()> {
    let root = root
        .canonicalize()
        .with_context(|| format!("resolve project root {}", root.display()))?;
    let input = input.unwrap_or_else(|| PathBuf::from("codex-audit-pipeline/.codex/flow.toml"));
    let input = resolve_inside(&root, input)?;
    let output = resolve_inside(
        &root,
        output.unwrap_or_else(|| PathBuf::from(DEFAULT_CONFIG_PATH)),
    )?;
    ensure_writable(&output, force)?;

    let source = fs::read_to_string(&input)
        .with_context(|| format!("read v1 workflow config {}", input.display()))?;
    let project_name = project_id(&root);
    let config = migrate_v1(&source, &project_name)?;
    if config.version != CONFIG_VERSION {
        bail!("migration did not produce schema v{CONFIG_VERSION}");
    }
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&output, toml::to_string_pretty(&config)?)?;
    println!("Migrated {} -> {}", input.display(), output.display());
    println!("The source file was not removed.");
    Ok(())
}

fn ensure_writable(path: &Path, force: bool) -> Result<()> {
    if path.exists() && !force {
        bail!(
            "{} already exists; pass --force to replace it",
            path.display()
        );
    }
    Ok(())
}

fn resolve_inside(root: &Path, path: PathBuf) -> Result<PathBuf> {
    let path = if path.is_absolute() {
        path
    } else {
        root.join(path)
    };
    if fs::symlink_metadata(&path).is_ok() {
        let resolved = path.canonicalize()?;
        if !resolved.starts_with(root) {
            bail!("path must remain inside the project: {}", path.display());
        }
        return Ok(path);
    }
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("path has no parent: {}", path.display()))?;
    let resolved_parent = if parent.exists() {
        parent.canonicalize()?
    } else {
        let mut existing = parent;
        while !existing.exists() {
            existing = existing
                .parent()
                .ok_or_else(|| anyhow::anyhow!("cannot resolve {}", path.display()))?;
        }
        existing.canonicalize()?
    };
    if !resolved_parent.starts_with(root) {
        bail!("path must remain inside the project: {}", path.display());
    }
    Ok(path)
}

fn project_id(root: &Path) -> String {
    let name = root
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("project");
    let mut id = String::new();
    for character in name.chars() {
        if character.is_ascii_alphanumeric() {
            id.push(character.to_ascii_lowercase());
        } else if !id.ends_with('-') {
            id.push('-');
        }
    }
    let id = id.trim_matches('-');
    if id.is_empty() {
        "project".into()
    } else {
        id.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_embedded_preset_is_valid() {
        for preset in PRESETS {
            FlowConfig::from_source(preset.flow)
                .unwrap_or_else(|error| panic!("preset {} is invalid: {error:#}", preset.name));
        }
    }

    #[test]
    fn project_names_become_portable_ids() {
        assert_eq!(project_id(Path::new("/tmp/My New_API")), "my-new-api");
    }

    #[cfg(unix)]
    #[test]
    fn existing_symlink_cannot_escape_project() {
        use std::os::unix::fs::symlink;

        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("arc-flow-path-test-{unique}"));
        let outside = std::env::temp_dir().join(format!("arc-flow-outside-{unique}"));
        fs::create_dir_all(&root).expect("create project fixture");
        fs::write(&outside, "outside").expect("create outside fixture");
        let link = root.join("flow.toml");
        symlink(&outside, &link).expect("create symlink fixture");

        let result = resolve_inside(&root.canonicalize().expect("canonical root"), link);
        fs::remove_file(&outside).ok();
        fs::remove_dir_all(&root).ok();

        assert!(result.is_err());
    }
}
