//! Job scheduler — matches jobs to workers and drives assignment via P2P.

pub mod matcher;

pub use matcher::{MatchRequest, WorkerCandidate};

use std::{
    collections::HashMap,
    sync::Arc,
    time::Duration,
};

use libp2p::PeerId;
use tokio::sync::mpsc;
use tracing::{info, warn, error};
use uuid::Uuid;

use crate::{
    db::Database,
    p2p::{JobCommand, WorkerEvent},
};
use matcher::select_workers;
use decentgpu_common::types::WorkerCapabilities;

/// Message from the API layer to the scheduler.
pub enum SchedulerCommand {
    /// A new job has been submitted and is ready for assignment.
    Enqueue {
        job_id:  Uuid,
        request: MatchRequest,
    },
}

/// Cheap handle to send commands to the running [`JobScheduler`].
#[derive(Clone)]
pub struct SchedulerHandle {
    tx: mpsc::Sender<SchedulerCommand>,
}

impl SchedulerHandle {
    pub async fn enqueue(&self, job_id: Uuid, request: MatchRequest) {
        self.tx.send(SchedulerCommand::Enqueue { job_id, request }).await.ok();
    }
}

/// Central job scheduler.
pub struct JobScheduler {
    db:           Arc<Database>,
    p2p_cmd_tx:   mpsc::Sender<JobCommand>,
    worker_rx:    mpsc::Receiver<WorkerEvent>,
    cmd_rx:       mpsc::Receiver<SchedulerCommand>,
    /// Root storage directory — code files are at `{storage_path}/{job_id}/code.py`.
    storage_path: Arc<String>,
    /// Live view of online workers.
    online:       HashMap<PeerId, WorkerCandidate>,
    /// pending[job_id] = list of peer_ids already tried.
    pending:      HashMap<Uuid, Vec<PeerId>>,
}

impl JobScheduler {
    /// Create the scheduler and a handle for the API layer.
    pub fn new(
        db:           Arc<Database>,
        p2p_cmd_tx:   mpsc::Sender<JobCommand>,
        worker_rx:    mpsc::Receiver<WorkerEvent>,
        storage_path: Arc<String>,
    ) -> (Self, SchedulerHandle) {
        let (cmd_tx, cmd_rx) = mpsc::channel(256);
        let handle = SchedulerHandle { tx: cmd_tx };
        let scheduler = Self {
            db,
            p2p_cmd_tx,
            worker_rx,
            cmd_rx,
            storage_path,
            online:  HashMap::new(),
            pending: HashMap::new(),
        };
        (scheduler, handle)
    }

    /// Run the scheduler event loop. Never returns (unless channels close).
    pub async fn run(mut self) {
        let mut retry_ticker = tokio::time::interval(Duration::from_secs(5));

        loop {
            tokio::select! {
                biased;

                // Commands from API layer (new jobs).
                Some(cmd) = self.cmd_rx.recv() => {
                    match cmd {
                        SchedulerCommand::Enqueue { job_id, request } => {
                            self.pending.insert(job_id, vec![]);
                            self.try_assign_one(job_id, &request).await;
                        }
                    }
                }

                // Worker lifecycle events from P2P layer.
                event = self.worker_rx.recv() => {
                    match event {
                        Some(WorkerEvent::Online { peer_id, capabilities }) => {
                            info!(peer_id = %peer_id, "worker came online");
                            self.online.insert(peer_id, WorkerCandidate {
                                peer_id,
                                capabilities: proto_caps_to_common(capabilities),
                                uptime_score:   100.0,
                                jobs_completed: 0,
                                is_busy:        false,
                            });
                            self.try_assign_pending().await;
                        }
                        Some(WorkerEvent::Offline { peer_id }) => {
                            info!(peer_id = %peer_id, "worker went offline");
                            self.online.remove(&peer_id);
                            self.handle_worker_offline(peer_id).await;
                        }
                        Some(WorkerEvent::Heartbeat { peer_id, uptime, is_busy, jobs_completed }) => {
                            if let Some(w) = self.online.get_mut(&peer_id) {
                                w.uptime_score   = uptime as f64;
                                w.is_busy        = is_busy;
                                w.jobs_completed = jobs_completed;
                            } else {
                                // Heartbeat from a worker that is not in the online map —
                                // this happens when the master marks a worker offline due
                                // to connection churn but the worker is still alive.
                                // Re-add with default capabilities so it can receive jobs.
                                // Capabilities will be overwritten if RegisterWorker is sent again.
                                info!(peer_id = %peer_id, "heartbeat from disconnected worker — re-adding to online pool");
                                let default_caps = decentgpu_common::types::WorkerCapabilities {
                                    gpus: vec![],
                                    cpu: decentgpu_common::types::CpuInfo {
                                        model: String::new(), cores: 1, threads: 1, freq_mhz: 0,
                                    },
                                    ram_mb: 0, disk_mb: 0,
                                    os: String::new(), worker_version: String::new(),
                                };
                                self.online.insert(peer_id, WorkerCandidate {
                                    peer_id,
                                    capabilities:   default_caps,
                                    uptime_score:   uptime as f64,
                                    jobs_completed,
                                    is_busy,
                                });
                                if !is_busy {
                                    self.try_assign_pending().await;
                                }
                            }
                        }
                        None => {
                            warn!("worker_rx closed — scheduler exiting");
                            break;
                        }
                    }
                }

                // Retry stale-pending jobs.
                _ = retry_ticker.tick() => {
                    self.retry_stale_pending().await;
                }
            }
        }
    }

    // ── Internal helpers ──────────────────────────────────────────────────────

    async fn try_assign_pending(&mut self) {
        let pending_jobs: Vec<Uuid> = self.pending.keys().copied().collect();
        for job_id in pending_jobs {
            // Fetch the job spec to get the match request.
            let row = match self.db.jobs.find_by_id(job_id).await {
                Ok(Some(r)) => r,
                _ => continue,
            };
            if row.status != "pending" {
                self.pending.remove(&job_id);
                continue;
            }
            let backend = row.gpu_backend.parse().unwrap_or(decentgpu_common::types::GpuBackend::CpuOnly);
            let request = MatchRequest {
                required_backend:  backend,
                memory_limit_mb:   row.memory_limit_mb.unwrap_or(512) as u64,
                max_duration_secs: row.max_duration_secs.unwrap_or(3600) as u64,
            };
            self.try_assign_one(job_id, &request).await;
        }
    }

    async fn try_assign_one(&mut self, job_id: Uuid, request: &MatchRequest) {
        let tried = self.pending.get(&job_id).cloned().unwrap_or_default();
        let candidates: Vec<WorkerCandidate> = self.online.values()
            .filter(|w| !tried.contains(&w.peer_id))
            .cloned()
            .collect();

        let selected = select_workers(&candidates, request);
        let peer_id = match selected.into_iter().next() {
            Some(p) => p,
            None => {
                warn!(%job_id, "no eligible workers, will retry");
                return;
            }
        };

        // Transition job to 'assigned' in DB.
        match self.db.jobs.transition_status(
            job_id, "pending", "assigned",
            Some(&peer_id.to_string()), None, None,
        ).await {
            Err(e) => {
                error!(?e, %job_id, "failed to transition job to assigned");
                return;
            }
            Ok(_) => {}
        }

        info!(%job_id, %peer_id, "assigning job to worker");

        // Build the assignment.
        let assignment = match build_job_assignment(&self.db, job_id, &peer_id, &self.storage_path.clone()).await {
            Ok(a) => a,
            Err(e) => {
                error!(?e, %job_id, "failed to build job assignment");
                return;
            }
        };

        // Send via P2P.
        self.p2p_cmd_tx.send(JobCommand::Assign {
            peer_id,
            job: Box::new(assignment),
        }).await.ok();

        // Record as tried.
        self.pending.entry(job_id).or_default().push(peer_id);
    }

    async fn retry_stale_pending(&mut self) {
        let pending_jobs: Vec<Uuid> = self.pending.keys().copied().collect();
        for job_id in pending_jobs {
            let row = match self.db.jobs.find_by_id(job_id).await {
                Ok(Some(r)) => r,
                _ => {
                    self.pending.remove(&job_id);
                    continue;
                }
            };
            if row.status != "pending" {
                self.pending.remove(&job_id);
                continue;
            }
            // Check if it's been pending for more than 30s (created_at is in row).
            let age = chrono::Utc::now() - row.created_at;
            if age.num_seconds() > 30 {
                let backend = row.gpu_backend.parse()
                    .unwrap_or(decentgpu_common::types::GpuBackend::CpuOnly);
                let request = MatchRequest {
                    required_backend:  backend,
                    memory_limit_mb:   row.memory_limit_mb.unwrap_or(512) as u64,
                    max_duration_secs: row.max_duration_secs.unwrap_or(3600) as u64,
                };
                self.try_assign_one(job_id, &request).await;
            }
        }
    }

    async fn handle_worker_offline(&mut self, peer_id: PeerId) {
        // Find any job assigned to this worker and mark it failed.
        if let Ok(rows) = self.db.jobs.list_by_hirer(
            uuid::Uuid::nil(), // we don't filter by hirer here - this is a workaround
            1000, 0,
        ).await {
            for row in rows {
                if row.worker_peer_id.as_deref() == Some(&peer_id.to_string())
                    && matches!(row.status.as_str(), "assigned" | "running")
                {
                    let _ = self.db.jobs.transition_status(
                        row.id, &row.status, "failed",
                        None, Some("worker went offline"), None,
                    ).await;
                    // Re-enqueue as pending.
                    self.pending.insert(row.id, vec![peer_id]);
                    // Reset back to pending in DB.
                    let _ = self.db.jobs.transition_status(
                        row.id, "failed", "pending",
                        None, None, None,
                    ).await;
                }
            }
        }
    }
}

// ── Helper functions ──────────────────────────────────────────────────────────

async fn build_job_assignment(
    db:           &Arc<Database>,
    job_id:       Uuid,
    peer_id:      &PeerId,
    storage_path: &str,
) -> anyhow::Result<decentgpu_proto::JobAssignment> {
    use std::path::PathBuf;

    let row = db.jobs.find_by_id(job_id).await?
        .ok_or_else(|| anyhow::anyhow!("job not found: {job_id}"))?;

    let backend_str = row.gpu_backend.as_str();
    let required_backend = match backend_str {
        "cuda"  => decentgpu_proto::GpuBackend::Cuda as i32,
        "metal" => decentgpu_proto::GpuBackend::Metal as i32,
        "rocm"  => decentgpu_proto::GpuBackend::Rocm as i32,
        _       => decentgpu_proto::GpuBackend::CpuOnly as i32,
    };

    // Read code and requirements from storage directory so we can inline them in the assignment.
    let job_dir       = PathBuf::from(storage_path).join(job_id.to_string());
    let code_bytes    = tokio::fs::read(job_dir.join("code.py")).await
        .unwrap_or_default();
    let req_bytes     = tokio::fs::read(job_dir.join("requirements.txt")).await
        .unwrap_or_default();

    let spec = decentgpu_proto::JobSpec {
        job_id:             job_id.to_string(),
        hirer_id:           row.hirer_id.to_string(),
        max_duration_secs:  row.max_duration_secs.unwrap_or(3600) as u64,
        memory_limit_mb:    row.memory_limit_mb.unwrap_or(512) as u64,
        cpu_limit_percent:  50,
        required_backend,
        image_hash:         String::new(),
        image_size_bytes:   0,
        code_bytes,
        requirements_bytes: req_bytes,
    };

    Ok(decentgpu_proto::JobAssignment {
        spec:           Some(spec),
        master_peer_id: peer_id.to_string(),
    })
}

fn proto_caps_to_common(caps: decentgpu_proto::WorkerCapabilities) -> WorkerCapabilities {
    use decentgpu_common::types::{CpuInfo, GpuInfo};
    WorkerCapabilities {
        gpus: caps.gpus.into_iter().map(|g| {
            let backend = match decentgpu_proto::GpuBackend::try_from(g.backend).unwrap_or_default() {
                decentgpu_proto::GpuBackend::Cuda    => decentgpu_common::types::GpuBackend::Cuda,
                decentgpu_proto::GpuBackend::Metal   => decentgpu_common::types::GpuBackend::Metal,
                decentgpu_proto::GpuBackend::Rocm    => decentgpu_common::types::GpuBackend::Rocm,
                decentgpu_proto::GpuBackend::CpuOnly => decentgpu_common::types::GpuBackend::CpuOnly,
            };
            GpuInfo { name: g.name, vram_mb: g.vram_mb, backend }
        }).collect(),
        cpu: caps.cpu.map(|c| CpuInfo {
            model:    c.model,
            cores:    c.cores,
            threads:  c.threads,
            freq_mhz: c.freq_mhz,
        }).unwrap_or(CpuInfo { model: String::new(), cores: 1, threads: 1, freq_mhz: 0 }),
        ram_mb:         caps.ram_mb,
        disk_mb:        caps.disk_mb,
        os:             caps.os,
        worker_version: caps.worker_version,
    }
}
