//! Job lifecycle state machine.

use anyhow::Result;
use bytes::Bytes;
use decentgpu_proto::JobAssignment;
use std::path::PathBuf;
use std::time::Instant;
use tracing::{info, instrument, warn};

use crate::docker::{cleanup, manager::DockerManager};

/// All possible states a job can be in on this worker.
#[derive(Debug)]
pub enum JobState {
    /// Waiting for work.
    Idle,
    /// Job assignment received from master; not yet accepted.
    AssignmentReceived { job: Box<JobAssignment> },
    /// Docker image tar is being received over the transfer stream.
    ReceivingImage {
        job_id: String,
        transfer_id: String,
        bytes_received: u64,
    },
    /// Image received; loading into Docker daemon.
    BuildingContainer { job_id: String },
    /// Container is running.
    Running {
        job_id: String,
        container_id: String,
    },
    /// Container exited successfully; uploading result.
    UploadingResult { job_id: String },
    /// Cleaning up resources before returning to Idle.
    Cleanup { job_id: String },
}

/// Context required to run a job.
pub struct JobContext {
    /// Docker manager.
    pub docker: DockerManager,
    /// Base temporary directory (job files stored under `{tmp_dir}/job-{id}/`).
    pub tmp_dir: PathBuf,
}

impl JobContext {
    /// Execute a job end-to-end.
    ///
    /// The caller provides the raw Docker image tar bytes and a log callback.
    #[instrument(skip(self, image_tar, on_log), fields(job_id = %job_id))]
    pub async fn run_job<F>(
        &self,
        job_id: &str,
        image_tar: Bytes,
        memory_limit_mb: u64,
        cpu_limit_percent: u64,
        max_duration_secs: u64,
        use_gpu: bool,
        mut on_log: F,
    ) -> Result<bool>
    where
        F: FnMut(String, bool) + Send,
    {
        let job_tmp = self.tmp_dir.join(format!("job-{job_id}"));
        tokio::fs::create_dir_all(&job_tmp).await?;

        let output_dir = job_tmp.join("output");
        tokio::fs::create_dir_all(&output_dir).await?;

        let output_dir_str = output_dir
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("non-UTF8 output path"))?
            .to_string();

        // Load image.
        info!("loading docker image");
        let image_id = self.docker.load_image(image_tar).await?;

        // Create container.
        info!("creating container");
        let container_id = self
            .docker
            .create_container(
                job_id,
                &image_id,
                memory_limit_mb,
                cpu_limit_percent,
                &output_dir_str,
                use_gpu,
            )
            .await?;

        // Start container.
        info!("starting container");
        self.docker.start_container(&container_id).await?;

        let start = Instant::now();

        // Stream logs with timeout.
        let docker_ref = &self.docker;
        let cid = container_id.clone();

        let _log_future = docker_ref.stream_logs(&cid, &mut on_log);
        let wait_future = docker_ref.wait_container(&cid);

        // Race logs stream and wait; enforce max duration.
        let timeout = tokio::time::Duration::from_secs(max_duration_secs.max(1));
        let exit_code = tokio::select! {
            _ = tokio::time::sleep(timeout) => {
                warn!(job_id, "job timed out");
                -1i64
            }
            result = wait_future => {
                result.unwrap_or(-1)
            }
        };

        info!(job_id, exit_code, elapsed_secs = start.elapsed().as_secs(), "container exited");

        // Cleanup.
        let _ = cleanup::cleanup_job(
            &self.docker,
            job_id,
            Some(&container_id),
            Some(&image_id),
            &self.tmp_dir,
        )
        .await;

        Ok(exit_code == 0)
    }
}
