//! Bollard-based Docker operations wrapper.

use anyhow::{Context, Result};
use bollard::{
    container::{
        Config, CreateContainerOptions, LogOutput, LogsOptions, RemoveContainerOptions,
        StartContainerOptions, StopContainerOptions, WaitContainerOptions,
    },
    image::ImportImageOptions,
    models::{DeviceRequest, HostConfig},
    Docker,
};
use bytes::Bytes;
use futures::StreamExt;
use std::collections::HashMap;
use tracing::{debug, info, instrument};

/// Wrapper around the bollard [`Docker`] client.
pub struct DockerManager {
    client: Docker,
}

impl DockerManager {
    /// Connect to the Docker daemon at the given socket path.
    pub fn new(socket_path: &str) -> Result<Self> {
        #[cfg(windows)]
        let client = if socket_path.starts_with("\\\\.\\pipe\\") {
            Docker::connect_with_named_pipe(socket_path, 120, bollard::API_DEFAULT_VERSION)
                .context("connect to Docker named pipe")?
        } else {
            Docker::connect_with_unix(socket_path, 120, bollard::API_DEFAULT_VERSION)
                .context("connect to Docker Unix socket")?
        };

        #[cfg(not(windows))]
        let client = Docker::connect_with_unix(socket_path, 120, bollard::API_DEFAULT_VERSION)
            .context("connect to Docker Unix socket")?;

        Ok(Self { client })
    }

    /// Load a Docker image from a tar byte stream.
    ///
    /// Returns the image ID (tag from the tar).
    #[instrument(skip(self, tar_data), fields(bytes = tar_data.len()))]
    pub async fn load_image(&self, tar_data: Bytes) -> Result<String> {
        let options = ImportImageOptions { quiet: true, ..Default::default() };
        let mut stream = self
            .client
            .import_image(options, tar_data, None);

        let mut image_id = String::new();
        while let Some(result) = stream.next().await {
            let item = result.context("image import stream error")?;
            if let Some(id) = item.id {
                image_id = id;
            }
        }

        info!(image_id = %image_id, "image loaded");
        Ok(image_id)
    }

    /// Create and start a container for a job.
    ///
    /// Returns the container ID.
    #[instrument(skip(self), fields(job_id = %job_id, image = %image_id))]
    pub async fn create_container(
        &self,
        job_id: &str,
        image_id: &str,
        memory_limit_mb: u64,
        cpu_limit_percent: u64,
        output_dir: &str,
        use_gpu: bool,
    ) -> Result<String> {
        let memory_bytes = (memory_limit_mb * 1024 * 1024) as i64;
        let nano_cpus = (cpu_limit_percent as i64) * 10_000_000;

        let binds = vec![format!("{output_dir}:/workspace/output:rw")];

        let mut device_requests = Vec::new();
        if use_gpu {
            device_requests.push(DeviceRequest {
                driver: Some("nvidia".into()),
                count: Some(-1), // all GPUs
                capabilities: Some(vec![vec!["gpu".into()]]),
                ..Default::default()
            });
        }

        let mut tmpfs = HashMap::new();
        tmpfs.insert("/tmp".to_string(), "rw,noexec,nosuid,size=512m".to_string());

        let host_config = HostConfig {
            memory: Some(memory_bytes),
            nano_cpus: Some(nano_cpus),
            readonly_rootfs: Some(true),
            tmpfs: Some(tmpfs),
            cap_drop: Some(vec!["ALL".into()]),
            security_opt: Some(vec!["no-new-privileges:true".into()]),
            binds: Some(binds),
            network_mode: Some("none".into()),
            device_requests: if use_gpu { Some(device_requests) } else { None },
            ..Default::default()
        };

        let name = format!("decentgpu-job-{job_id}");
        let config: Config<String> = Config {
            image: Some(image_id.to_string()),
            working_dir: Some("/workspace".to_string()),
            host_config: Some(host_config),
            ..Default::default()
        };

        let container = self
            .client
            .create_container(
                Some(CreateContainerOptions {
                    name: name.clone(),
                    ..Default::default()
                }),
                config,
            )
            .await
            .context("create container")?;

        info!(container_id = %container.id, "container created");
        Ok(container.id)
    }

    /// Start a container.
    #[instrument(skip(self), fields(container_id = %container_id))]
    pub async fn start_container(&self, container_id: &str) -> Result<()> {
        self.client
            .start_container(container_id, None::<StartContainerOptions<String>>)
            .await
            .context("start container")?;
        info!("container started");
        Ok(())
    }

    /// Stream logs from a container, yielding each line.
    #[instrument(skip(self, on_log), fields(container_id = %container_id))]
    pub async fn stream_logs<F>(
        &self,
        container_id: &str,
        mut on_log: F,
    ) -> Result<()>
    where
        F: FnMut(String, bool) + Send, // (message, is_stderr)
    {
        let options = LogsOptions::<String> {
            follow: true,
            stdout: true,
            stderr: true,
            ..Default::default()
        };

        let mut stream = self.client.logs(container_id, Some(options));
        while let Some(result) = stream.next().await {
            match result {
                Ok(LogOutput::StdOut { message }) => {
                    on_log(String::from_utf8_lossy(&message).to_string(), false);
                }
                Ok(LogOutput::StdErr { message }) => {
                    on_log(String::from_utf8_lossy(&message).to_string(), true);
                }
                Ok(_) => {}
                Err(e) => {
                    debug!(error = %e, "log stream error");
                    break;
                }
            }
        }
        Ok(())
    }

    /// Wait for a container to exit and return its exit code.
    #[instrument(skip(self), fields(container_id = %container_id))]
    pub async fn wait_container(&self, container_id: &str) -> Result<i64> {
        let mut stream = self
            .client
            .wait_container(container_id, None::<WaitContainerOptions<String>>);

        let mut exit_code = 0i64;
        while let Some(result) = stream.next().await {
            let status = result.context("wait container")?;
            exit_code = status.status_code;
        }
        info!(exit_code, "container exited");
        Ok(exit_code)
    }

    /// Stop and remove a container.
    #[instrument(skip(self), fields(container_id = %container_id))]
    pub async fn remove_container(&self, container_id: &str) -> Result<()> {
        let _ = self
            .client
            .stop_container(container_id, Some(StopContainerOptions { t: 5 }))
            .await;

        self.client
            .remove_container(
                container_id,
                Some(RemoveContainerOptions {
                    force: true,
                    v: true,
                    ..Default::default()
                }),
            )
            .await
            .context("remove container")?;
        info!("container removed");
        Ok(())
    }

    /// Remove a Docker image.
    #[instrument(skip(self), fields(image_id = %image_id))]
    pub async fn remove_image(&self, image_id: &str) -> Result<()> {
        self.client
            .remove_image(image_id, None, None)
            .await
            .context("remove image")?;
        info!("image removed");
        Ok(())
    }
}
