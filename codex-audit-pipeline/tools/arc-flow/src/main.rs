mod audit;
mod doctor;
mod process;
mod project;
mod scope;
mod secrets;
mod verify;

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};
use project::Project;
use scope::{Component, ScopeMode};
use std::path::PathBuf;
use std::process::ExitCode;
use verify::Profile;

#[derive(Debug, Parser)]
#[command(
    name = "arc-flow",
    version,
    about = "Development workflow and architecture guard for arc-admin",
    arg_required_else_help = true,
    after_help = "Examples:\n  cargo flow doctor\n  cargo flow scope\n  cargo flow verify\n  cargo flow verify --all\n  cargo flow verify --components backend,frontend"
)]
struct Cli {
    /// Override automatic project root discovery.
    #[arg(long, global = true, value_name = "PATH")]
    project_root: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Check local tools, configuration, Git, and test database access.
    Doctor {
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,
        /// Treat warnings as failures.
        #[arg(long)]
        strict: bool,
    },
    /// Show changed files and the verification components they select.
    Scope {
        #[command(flatten)]
        scope: ScopeArgs,
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Scan file names for high-confidence credential patterns.
    Secrets {
        /// Scan the staged snapshot instead of the working tree.
        #[arg(long)]
        staged: bool,
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Run deterministic architecture rules and write review_context reports.
    Audit {
        /// Emit the complete audit report as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Run secrets, audit, lint, compile, tests, and frontend build.
    #[command(visible_alias = "check")]
    Verify {
        #[command(flatten)]
        scope: ScopeArgs,
        /// Override scope detection with a comma-separated component list.
        #[arg(
            long,
            value_enum,
            value_delimiter = ',',
            num_args = 1..,
            conflicts_with_all = ["staged", "all", "base"]
        )]
        components: Vec<Component>,
    },
    /// Run the staged, fast verification profile used by pre-commit.
    Hook,
    /// Extract one trace's error context from JSON Lines logs.
    ParseLogs {
        #[arg(short, long)]
        input: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },
}

#[derive(Debug, Clone, Args)]
struct ScopeArgs {
    /// Inspect only staged files.
    #[arg(long, conflicts_with_all = ["all", "base"])]
    staged: bool,
    /// Select every verification component.
    #[arg(long, conflicts_with_all = ["staged", "base"])]
    all: bool,
    /// Inspect committed changes in REF...HEAD.
    #[arg(long, value_name = "REF", conflicts_with_all = ["staged", "all"])]
    base: Option<String>,
}

impl ScopeArgs {
    fn mode(&self) -> ScopeMode {
        if self.staged {
            ScopeMode::Staged
        } else if self.all {
            ScopeMode::All
        } else if let Some(reference) = &self.base {
            ScopeMode::Base(reference.clone())
        } else {
            ScopeMode::WorkingTree
        }
    }
}

fn main() -> ExitCode {
    process::install_signal_handlers();
    match run() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(error) => {
            eprintln!("ERROR: {error:#}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<bool> {
    let cli = Cli::parse();
    let project = Project::discover(cli.project_root)?;
    project.prepare()?;

    match cli.command {
        Commands::Doctor { json, strict } => {
            let report = doctor::run(&project)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                report.print();
            }
            Ok(report.failures == 0 && (!strict || report.warnings == 0))
        }
        Commands::Scope { scope: args, json } => {
            let result = scope::detect(&project, &args.mode())?;
            result.write_reports(&project)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                print_scope(&result);
            }
            Ok(true)
        }
        Commands::Secrets { staged, json } => {
            let mode = if staged {
                secrets::SecretMode::Staged
            } else {
                secrets::SecretMode::WorkingTree
            };
            let findings = secrets::scan(&project, mode)?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "passed": findings.is_empty(),
                        "findings": findings,
                    }))?
                );
            } else if findings.is_empty() {
                println!("Secret scan passed");
            } else {
                eprintln!("Secret scan failed in {} file(s):", findings.len());
                for file in &findings {
                    eprintln!("  {file}");
                }
                eprintln!("Remove and revoke each credential before continuing.");
            }
            Ok(findings.is_empty())
        }
        Commands::Audit { json } => {
            let outcome = audit::run(&project.audit_config, &project.reports, json)?;
            if !json {
                println!(
                    "Audit: {} violation(s), {} blocker(s), {} error(s)",
                    outcome.total_violations, outcome.blocker_count, outcome.error_count
                );
                println!(
                    "Report: {}",
                    project.reports.join("review_context.json").display()
                );
            }
            Ok(outcome.total_violations == 0)
        }
        Commands::Verify {
            scope: args,
            components,
        } => {
            let selected = if components.is_empty() {
                scope::detect(&project, &args.mode())?
            } else {
                verify::explicit_scope(&components)
            };
            Ok(verify::run(&project, selected, Profile::Full)?.passed)
        }
        Commands::Hook => {
            let selected = scope::detect(&project, &ScopeMode::Staged)?;
            Ok(verify::run(&project, selected, Profile::Hook)?.passed)
        }
        Commands::ParseLogs { input, output } => {
            audit::parse_logs(&input, &output)
                .with_context(|| format!("parse log file {}", input.display()))?;
            println!("Error context: {}", output.display());
            Ok(true)
        }
    }
}

fn print_scope(scope: &scope::ScopeResult) {
    println!("Scope: {}", scope.mode);
    println!("Changed files: {}", scope.changed_files.len());
    for file in &scope.changed_files {
        println!("  {file}");
    }
    let components = scope
        .components
        .iter()
        .map(|component| component.label())
        .collect::<Vec<_>>()
        .join(", ");
    println!(
        "Components: {}",
        if components.is_empty() {
            "none"
        } else {
            &components
        }
    );
}
