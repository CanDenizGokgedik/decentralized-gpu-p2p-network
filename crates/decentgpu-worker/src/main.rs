#![deny(clippy::all)]

//! DecentGPU Worker Node — runs on provider machines to execute jobs.

pub mod docker;
pub mod executor;
pub mod gpu;
pub mod p2p;

use anyhow::Result;
use decentgpu_common::config::{load_config, WorkerConfig};
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, BufReader};
use tracing::{info, warn, error};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("decentgpu_worker=debug".parse()?)
                .add_directive("libp2p=info".parse()?),
        )
        .json()
        .init();

    let cfg: WorkerConfig = load_config("WORKER")?;
    info!("worker node starting");

    // ── Detect GPU capabilities ────────────────────────────────────────────
    let capabilities = gpu::detect().await?;
    info!(
        gpus = capabilities.gpus.len(),
        ram_mb = capabilities.ram_mb,
        cpu_cores = capabilities.cpu.cores,
        "capabilities detected"
    );

    // ── Probe Docker availability once at startup ──────────────────────────
    let docker_available = probe_docker().await;
    if docker_available {
        info!("Docker daemon reachable — jobs will run in isolated containers");
    } else {
        warn!("Docker daemon not reachable — falling back to direct python3 execution (no isolation)");
    }

    // ── Start P2P layer ────────────────────────────────────────────────────
    let (p2p_handle, mut job_rx, result_tx) = p2p::start(cfg, capabilities).await?;

    // ── Job executor: receive assignments, run code, report results ────────
    tokio::spawn(async move {
        while let Some(assignment) = job_rx.recv().await {
            let spec = match assignment.spec.as_ref() {
                Some(s) => s,
                None => {
                    warn!("received JobAssignment without spec, skipping");
                    continue;
                }
            };

            let job_id       = spec.job_id.clone();
            let code_bytes   = spec.code_bytes.clone();
            let req_bytes    = spec.requirements_bytes.clone();
            let max_dur_secs = spec.max_duration_secs.max(10);
            let memory_mb    = spec.memory_limit_mb.max(128);

            info!(
                job_id = %job_id,
                code_size  = code_bytes.len(),
                req_size   = req_bytes.len(),
                max_dur    = max_dur_secs,
                memory_mb,
                "executing job"
            );

            if code_bytes.is_empty() {
                warn!(job_id = %job_id, "job has no code — marking failed");
                let _ = result_tx.send(p2p::handler::JobResult {
                    job_id,
                    success: false,
                    output: "no code bytes in assignment".into(),
                }).await;
                continue;
            }

            // Write code and requirements to a unique temp directory.
            let job_dir = std::env::temp_dir().join(format!("decentgpu-{}", &job_id[..8]));
            if let Err(e) = tokio::fs::create_dir_all(&job_dir).await {
                error!(job_id = %job_id, err = %e, "failed to create job temp dir");
                let _ = result_tx.send(p2p::handler::JobResult {
                    job_id,
                    success: false,
                    output: format!("temp dir error: {e}"),
                }).await;
                continue;
            }

            let code_path = job_dir.join("code.py");
            let req_path  = job_dir.join("requirements.txt");

            if let Err(e) = tokio::fs::write(&code_path, &code_bytes).await {
                error!(job_id = %job_id, err = %e, "failed to write code.py");
                let _ = result_tx.send(p2p::handler::JobResult {
                    job_id,
                    success: false,
                    output: format!("write code error: {e}"),
                }).await;
                continue;
            }
            let has_req = !req_bytes.is_empty();
            if has_req {
                let _ = tokio::fs::write(&req_path, &req_bytes).await;
            }

            // ── Streaming execution with mpsc channel ──────────────────────
            let (log_tx, mut log_rx) = tokio::sync::mpsc::channel::<String>(1024);

            let job_id_exec  = job_id.clone();
            let job_dir_exec = job_dir.clone();
            let req_path_exec = req_path.clone();

            let exec_handle = if docker_available {
                let log_tx_d = log_tx.clone();
                tokio::spawn(async move {
                    run_in_docker(
                        job_id_exec,
                        job_dir_exec,
                        has_req,
                        max_dur_secs,
                        memory_mb,
                        log_tx_d,
                    ).await
                })
            } else {
                let log_tx_p = log_tx.clone();
                tokio::spawn(async move {
                    run_python3_direct(
                        job_id_exec,
                        job_dir_exec,
                        req_path_exec,
                        has_req,
                        max_dur_secs,
                        log_tx_p,
                    ).await
                })
            };

            // Drop our own clone of log_tx so the channel closes when the spawned task finishes.
            drop(log_tx);

            // Collect lines while execution runs.
            let mut log_lines: Vec<String> = Vec::new();
            while let Some(line) = log_rx.recv().await {
                log_lines.push(line);
            }

            // Await exec result (error message; output went through channel).
            let (success, error_msg) = match exec_handle.await {
                Ok(r) => r,
                Err(e) => (false, format!("task join error: {e}")),
            };

            let output = if success || error_msg.is_empty() {
                log_lines.join("\n")
            } else {
                let base = log_lines.join("\n");
                if base.is_empty() {
                    error_msg.clone()
                } else {
                    format!("{}\n{}", base, error_msg)
                }
            };

            // Clean up temp dir.
            let _ = tokio::fs::remove_dir_all(&job_dir).await;

            info!(job_id = %job_id, success, "job finished, sending result");
            let _ = result_tx.send(p2p::handler::JobResult {
                job_id,
                success,
                output,
            }).await;
        }
    });

    p2p_handle.await??;
    Ok(())
}

// ── Execution helpers ─────────────────────────────────────────────────────────

/// Run code inside an isolated Docker container.
///
/// Builds a fresh image from the job directory, runs it, captures logs, cleans up.
async fn run_in_docker(
    job_id:       String,
    job_dir:      PathBuf,
    has_req:      bool,
    max_dur_secs: u64,
    memory_mb:    u64,
    log_tx:       tokio::sync::mpsc::Sender<String>,
) -> (bool, String) {
    use bollard::Docker;
    use bollard::image::BuildImageOptions;
    use bollard::container::{
        Config, CreateContainerOptions, LogsOptions,
        RemoveContainerOptions, StartContainerOptions, WaitContainerOptions,
    };
    use bollard::models::HostConfig;
    use futures::StreamExt as _;

    let job_id = job_id.as_str();

    // ── Build image ────────────────────────────────────────────────────────
    let dockerfile_content = docker::builder::generate_python_dockerfile(has_req);
    if let Err(e) = tokio::fs::write(job_dir.join("Dockerfile"), &dockerfile_content).await {
        error!(job_id, err = %e, "failed to write Dockerfile");
        return (false, format!("Dockerfile write error: {e}"));
    }

    let tag = format!("decentgpu-job-{}:latest", &job_id[..8]);
    info!(job_id, %tag, "building Docker image");

    // Tar the build context in a blocking task to avoid blocking the async runtime.
    let dir = job_dir.clone();
    let context_bytes = match tokio::task::spawn_blocking(move || tar_dir_sync(&dir)).await {
        Ok(Ok(b)) => b,
        Ok(Err(e)) => return (false, format!("tar error: {e}")),
        Err(e)    => return (false, format!("spawn_blocking error: {e}")),
    };

    let client = match Docker::connect_with_local_defaults() {
        Ok(c)  => c,
        Err(e) => return (false, format!("docker connect: {e}")),
    };

    let build_opts = BuildImageOptions {
        dockerfile: "Dockerfile",
        t:          &tag,
        rm:         true,
        ..Default::default()
    };
    let mut build_stream = client.build_image(
        build_opts,
        None,
        Some(bytes::Bytes::from(context_bytes).into()),
    );
    while let Some(item) = build_stream.next().await {
        match item {
            Ok(info) => {
                if let Some(s) = &info.stream {
                    tracing::debug!(job_id, output = s.trim_end(), "docker build");
                }
                if let Some(err) = info.error {
                    error!(job_id, docker_err = %err, "docker build failed");
                    return (false, format!("docker build error: {err}"));
                }
            }
            Err(e) => return (false, format!("build stream error: {e}")),
        }
    }
    info!(job_id, "Docker image built");

    // ── Create container ───────────────────────────────────────────────────
    let container_name = format!("dg-{}", &job_id[..8]);
    let output_dir = job_dir.join("output");
    let _ = tokio::fs::create_dir_all(&output_dir).await;

    let host_cfg = HostConfig {
        memory:          Some((memory_mb * 1024 * 1024) as i64),
        memory_swap:     Some((memory_mb * 1024 * 1024) as i64),
        network_mode:    Some("none".to_string()),
        readonly_rootfs: Some(false),
        cap_drop:        Some(vec!["ALL".to_string()]),
        security_opt:    Some(vec!["no-new-privileges:true".to_string()]),
        binds:           Some(vec![
            format!("{}:/workspace/output:rw", output_dir.to_string_lossy())
        ]),
        ..Default::default()
    };
    let config = Config {
        image:       Some(tag.clone()),
        working_dir: Some("/workspace".to_string()),
        host_config: Some(host_cfg),
        ..Default::default()
    };

    let container = match client.create_container(
        Some(CreateContainerOptions { name: &container_name, platform: None }),
        config,
    ).await {
        Ok(c)  => { info!(job_id, container_id = %c.id, "container created"); c }
        Err(e) => {
            let _ = client.remove_image(&tag, None, None).await;
            return (false, format!("create container: {e}"));
        }
    };
    let container_id = container.id;

    // ── Start ──────────────────────────────────────────────────────────────
    if let Err(e) = client.start_container(
        &container_id,
        None::<StartContainerOptions<String>>,
    ).await {
        let _ = client.remove_container(&container_id, Some(RemoveContainerOptions { force: true, ..Default::default() })).await;
        let _ = client.remove_image(&tag, None, None).await;
        return (false, format!("start container: {e}"));
    }
    info!(job_id, "container started");

    // ── Wait with timeout ──────────────────────────────────────────────────
    let timeout  = std::time::Duration::from_secs(max_dur_secs);
    let cid      = container_id.clone();
    let wait_fut = async {
        let mut s = client.wait_container(
            &cid,
            Some(WaitContainerOptions { condition: "not-running" }),
        );
        s.next().await
    };

    let (success, error_msg) = match tokio::time::timeout(timeout, wait_fut).await {
        Ok(Some(Ok(r))) => {
            let code = r.status_code;
            info!(job_id, exit_code = code, "container exited");
            (code == 0, if code == 0 { String::new() } else { format!("container exited with code {code}") })
        }
        Ok(Some(Err(e))) => (false, e.to_string()),
        Ok(None)         => (false, "wait stream ended unexpectedly".into()),
        Err(_)           => {
            warn!(job_id, max_dur_secs, "job timed out — killing container");
            let _ = client.kill_container(&container_id, None::<bollard::container::KillContainerOptions<String>>).await;
            (false, format!("job timed out after {max_dur_secs}s"))
        }
    };

    // Collect logs and send via log_tx.
    let log_opts = LogsOptions::<String> {
        stdout: true,
        stderr: true,
        tail: "500".to_string(),
        ..Default::default()
    };
    let mut log_stream = client.logs(&container_id, Some(log_opts));
    while let Some(Ok(line)) = log_stream.next().await {
        let s = line.to_string();
        let trimmed = s.trim();
        if !trimmed.is_empty() {
            log_tx.send(trimmed.to_string()).await.ok();
        }
    }

    // ── Cleanup ────────────────────────────────────────────────────────────
    let _ = client.remove_container(
        &container_id,
        Some(RemoveContainerOptions { force: true, ..Default::default() }),
    ).await;
    let _ = client.remove_image(&tag, None, None).await;
    info!(job_id, success, "Docker job complete, container removed");

    (success, error_msg)
}

/// Fallback: run code directly with `python3` (no container isolation).
async fn run_python3_direct(
    job_id:       String,
    job_dir:      PathBuf,
    req_path:     PathBuf,
    has_req:      bool,
    max_dur_secs: u64,
    log_tx:       tokio::sync::mpsc::Sender<String>,
) -> (bool, String) {
    let job_id_str = job_id.as_str();

    // Install requirements if present.
    if has_req {
        info!(job_id = job_id_str, "installing requirements via pip3");
        let pip_out = tokio::process::Command::new("pip3")
            .args(["install", "-q", "-r", req_path.to_str().unwrap_or("requirements.txt")])
            .output()
            .await;
        match pip_out {
            Ok(o) if o.status.success() => info!(job_id = job_id_str, "requirements installed"),
            Ok(o) => warn!(
                job_id = job_id_str,
                stderr = %String::from_utf8_lossy(&o.stderr).trim(),
                "pip install had warnings (continuing)"
            ),
            Err(e) => warn!(job_id = job_id_str, err = %e, "pip3 not found — continuing without install"),
        }
    }

    let code_path = job_dir.join("code.py");
    info!(job_id = job_id_str, max_dur_secs, "spawning python3 (no Docker — direct execution)");

    let mut child = match tokio::process::Command::new("python3")
        .arg("-u")
        .arg(code_path.to_str().unwrap_or("code.py"))
        .current_dir(&job_dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            error!(job_id = job_id_str, err = %e, "python3 spawn failed");
            return (false, format!("spawn error: {e}"));
        }
    };

    let stdout = child.stdout.take().expect("stdout not captured");
    let stderr = child.stderr.take().expect("stderr not captured");

    // Spawn stderr reader.
    let log_tx2 = log_tx.clone();
    tokio::spawn(async move {
        let mut stderr_lines = BufReader::new(stderr).lines();
        while let Ok(Some(l)) = stderr_lines.next_line().await {
            log_tx2.send(format!("[stderr] {}", l)).await.ok();
        }
    });

    // Read stdout with timeout.
    let timeout = tokio::time::Duration::from_secs(max_dur_secs);
    let mut stdout_lines = BufReader::new(stdout).lines();
    let timed_out = tokio::time::timeout(timeout, async {
        while let Ok(Some(line)) = stdout_lines.next_line().await {
            log_tx.send(line).await.ok();
        }
    }).await.is_err();

    if timed_out {
        warn!(job_id = job_id_str, max_dur_secs, "job timed out");
        let _ = child.kill().await;
        log_tx.send("\u{26a0}\u{fe0f} Zaman a\u{15f}\u{131}m\u{131}".to_string()).await.ok();
        return (false, "timeout".to_string());
    }

    let status = match child.wait().await {
        Ok(s) => s,
        Err(e) => {
            error!(job_id = job_id_str, err = %e, "child.wait() failed");
            return (false, format!("wait error: {e}"));
        }
    };

    info!(
        job_id = job_id_str,
        exit_code = ?status.code(),
        "python3 execution complete"
    );

    let error_string = if status.success() {
        String::new()
    } else {
        format!("python3 exited with status: {status}")
    };

    (status.success(), error_string)
}

/// Check whether the Docker daemon is reachable.
async fn probe_docker() -> bool {
    use bollard::Docker;
    match Docker::connect_with_local_defaults() {
        Ok(d)  => d.ping().await.is_ok(),
        Err(_) => false,
    }
}

/// Build a tar archive of a directory (blocking — call from spawn_blocking).
fn tar_dir_sync(dir: &std::path::Path) -> anyhow::Result<Vec<u8>> {
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            let name = entry.file_name().to_string_lossy().to_string();
            let data = std::fs::read(entry.path())?;
            entries.push((name, data));
        }
    }
    let mut ar = tar::Builder::new(Vec::new());
    for (name, data) in &entries {
        let mut header = tar::Header::new_gnu();
        header.set_size(data.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        ar.append_data(&mut header, name, data.as_slice())?;
    }
    Ok(ar.into_inner()?)
}
