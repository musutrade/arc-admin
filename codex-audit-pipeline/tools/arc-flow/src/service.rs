use crate::config::ServiceConfig;
use crate::project::Project;
use anyhow::{bail, Context, Result};
use std::collections::BTreeMap;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub struct ServiceManager<'a> {
    project: &'a Project,
    running: BTreeMap<String, RunningService>,
}

impl<'a> ServiceManager<'a> {
    pub fn new(project: &'a Project) -> Self {
        Self {
            project,
            running: BTreeMap::new(),
        }
    }

    pub fn environment(&mut self, id: &str) -> Result<(String, String)> {
        if !self.running.contains_key(id) {
            let config = self
                .project
                .config
                .service(id)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("unknown service {id:?}"))?;
            let running = RunningService::start(self.project, id, config)?;
            self.running.insert(id.to_string(), running);
        }
        let service = self.running.get(id).expect("service inserted");
        Ok((service.inject_env.clone(), service.value.clone()))
    }
}

struct RunningService {
    inject_env: String,
    value: String,
    container: Option<String>,
}

impl RunningService {
    fn start(project: &Project, id: &str, config: ServiceConfig) -> Result<Self> {
        match config {
            ServiceConfig::Environment {
                source_env,
                inject_env,
            } => {
                let value = std::env::var(&source_env)
                    .with_context(|| format!("service {id:?} requires {source_env}"))?;
                Ok(Self {
                    inject_env,
                    value,
                    container: None,
                })
            }
            ServiceConfig::Docker {
                image,
                external_env,
                inject_env,
                startup_timeout_secs,
                container_port,
                environment,
                healthcheck,
                connection,
                ..
            } => {
                if let Some((_, value)) = external_env
                    .as_ref()
                    .and_then(|name| std::env::var(name).ok().map(|value| (name, value)))
                {
                    return Ok(Self {
                        inject_env,
                        value,
                        container: None,
                    });
                }
                ensure_docker(project, id)?;
                start_docker(
                    project,
                    id,
                    image,
                    inject_env,
                    startup_timeout_secs,
                    container_port,
                    environment,
                    healthcheck,
                    connection,
                )
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn start_docker(
    project: &Project,
    id: &str,
    image: String,
    inject_env: String,
    startup_timeout_secs: u64,
    container_port: u16,
    environment: BTreeMap<String, String>,
    healthcheck: Vec<String>,
    connection: String,
) -> Result<RunningService> {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let name = format!(
        "arc-flow-{}-{id}-{}-{unique}",
        project.config.project.name,
        std::process::id()
    );
    let publish = format!("127.0.0.1::{container_port}");
    let mut args = vec![
        "run".to_string(),
        "--rm".into(),
        "--detach".into(),
        "--pull=never".into(),
        "--name".into(),
        name.clone(),
    ];
    for (key, value) in environment {
        args.extend(["--env".into(), format!("{key}={value}")]);
    }
    args.extend(["--publish".into(), publish, image.clone()]);
    let output = Command::new("docker")
        .current_dir(&project.root)
        .args(&args)
        .output()
        .with_context(|| format!("start Docker service {id:?}"))?;
    if !output.status.success() {
        bail!(
            "failed to start Docker service {id:?} with image {image}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    let mut running = RunningService {
        inject_env,
        value: String::new(),
        container: Some(name),
    };
    let deadline = Instant::now() + Duration::from_secs(startup_timeout_secs);
    while Instant::now() < deadline {
        if crate::process::cancelled() {
            bail!("verification cancelled while waiting for service {id:?}");
        }
        if let Some(port) = running.port(container_port)? {
            let mut health_args = vec![
                "exec".to_string(),
                running.container.as_deref().unwrap_or_default().to_string(),
            ];
            health_args.extend(healthcheck.iter().cloned());
            let ready = Command::new("docker")
                .args(&health_args)
                .output()
                .is_ok_and(|value| value.status.success());
            if ready {
                running.value = connection.replace("{host_port}", &port);
                return Ok(running);
            }
        }
        thread::sleep(Duration::from_secs(1));
    }
    bail!("service {id:?} did not become ready within {startup_timeout_secs} seconds")
}

fn ensure_docker(project: &Project, id: &str) -> Result<()> {
    let info = Command::new("docker")
        .current_dir(&project.root)
        .arg("info")
        .output()
        .with_context(|| format!("Docker is required by service {id:?}"))?;
    if !info.status.success() {
        bail!("Docker daemon is unavailable for service {id:?}");
    }
    Ok(())
}

impl RunningService {
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

impl Drop for RunningService {
    fn drop(&mut self) {
        if let Some(container) = &self.container {
            let _ = Command::new("docker")
                .args(["rm", "--force", container])
                .output();
        }
    }
}

pub fn check_available(project: &Project, id: &str) -> Result<String> {
    let service = project
        .config
        .service(id)
        .ok_or_else(|| anyhow::anyhow!("unknown service {id:?}"))?;
    match service {
        ServiceConfig::Environment { source_env, .. } => {
            std::env::var_os(source_env)
                .ok_or_else(|| anyhow::anyhow!("{source_env} is not configured"))?;
            Ok(format!("{source_env} is configured"))
        }
        ServiceConfig::Docker {
            image,
            external_env,
            ..
        } => {
            if let Some(name) = external_env {
                if std::env::var_os(name).is_some() {
                    return Ok(format!("{name} is configured"));
                }
            }
            ensure_docker(project, id)?;
            let image_ready = Command::new("docker")
                .args(["image", "inspect", image])
                .output()
                .is_ok_and(|output| output.status.success());
            if !image_ready {
                bail!("Docker image {image} is not available; run `docker pull {image}`");
            }
            Ok(format!("Docker and {image} ready"))
        }
    }
}
