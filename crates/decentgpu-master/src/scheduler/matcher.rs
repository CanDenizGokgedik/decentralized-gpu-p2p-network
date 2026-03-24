//! Pure worker-selection algorithm — no I/O, fully unit-testable.

use libp2p::PeerId;
use decentgpu_common::types::{GpuBackend, WorkerCapabilities};

/// An online worker candidate for job assignment.
#[derive(Debug, Clone)]
pub struct WorkerCandidate {
    pub peer_id:        PeerId,
    pub capabilities:   WorkerCapabilities,
    pub uptime_score:   f64,   // 0–100
    pub jobs_completed: u64,
    pub is_busy:        bool,
}

/// Criteria a job requires from a worker.
#[derive(Debug, Clone)]
pub struct MatchRequest {
    pub required_backend:  GpuBackend,
    pub memory_limit_mb:   u64,
    pub max_duration_secs: u64,
}

/// Select up to 3 best-matching workers for a job.
///
/// Pure function — no async, no DB access. Easily unit-testable.
///
/// Scoring (weights sum to 1.0):
/// - 0.40 × uptime component  (uptime_score / 100)
/// - 0.30 × jobs component    (jobs_completed / max_jobs, normalised)
/// - 0.30 × resource match    (see `resource_match_score`)
pub fn select_workers(
    candidates: &[WorkerCandidate],
    request:    &MatchRequest,
) -> Vec<PeerId> {
    // Normalisation denominator for jobs_completed.
    let max_jobs = candidates
        .iter()
        .map(|c| c.jobs_completed)
        .max()
        .unwrap_or(1)
        .max(1) as f64;

    let mut scored: Vec<(f64, PeerId)> = candidates
        .iter()
        .filter_map(|c| {
            // Hard filter: worker must be idle.
            if c.is_busy {
                return None;
            }
            // Hard filter: backend match.
            if !backend_matches(c, request.required_backend) {
                return None;
            }
            // Hard filter: VRAM (skip for CPU-only).
            if request.required_backend != GpuBackend::CpuOnly
                && !has_enough_vram(c, request.memory_limit_mb)
            {
                return None;
            }

            let uptime_component   = (c.uptime_score / 100.0).clamp(0.0, 1.0) * 0.40;
            let jobs_component     = (c.jobs_completed as f64 / max_jobs).min(1.0) * 0.30;
            let resource_component = resource_match_score(c, request) * 0.30;

            let total = uptime_component + jobs_component + resource_component;
            Some((total, c.peer_id))
        })
        .collect();

    // Sort descending by score.
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(3);
    scored.into_iter().map(|(_, pid)| pid).collect()
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn backend_matches(candidate: &WorkerCandidate, required: GpuBackend) -> bool {
    if required == GpuBackend::CpuOnly {
        return true; // Every worker supports CPU
    }
    candidate.capabilities.gpus.iter().any(|g| g.backend == required)
}

fn has_enough_vram(candidate: &WorkerCandidate, memory_limit_mb: u64) -> bool {
    candidate
        .capabilities
        .gpus
        .iter()
        .any(|g| g.vram_mb >= memory_limit_mb)
}

/// Score in [0.0, 1.0] based on how well the worker's resources fit the job.
///
/// For CPU-only jobs, returns 1.0 (no VRAM constraint).
/// For GPU jobs: score is highest when utilization ≈ 0.7 (prefer workers
/// where the job uses 70% of their VRAM — not wasteful, not too tight).
fn resource_match_score(candidate: &WorkerCandidate, request: &MatchRequest) -> f64 {
    if request.required_backend == GpuBackend::CpuOnly {
        return 1.0;
    }

    // Find the best matching GPU (smallest that still fits).
    let available_vram = candidate
        .capabilities
        .gpus
        .iter()
        .filter(|g| g.vram_mb >= request.memory_limit_mb)
        .map(|g| g.vram_mb)
        .min()
        .unwrap_or(0);

    if available_vram == 0 {
        return 0.0;
    }

    let utilization = request.memory_limit_mb as f64 / available_vram as f64;
    // Sweet spot at 0.7; penalise deviation.
    let score = 1.0 - (utilization - 0.7).abs() * 2.0;
    score.clamp(0.0, 1.0)
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use decentgpu_common::types::{CpuInfo, GpuInfo};

    fn cpu_worker(peer_id: PeerId, uptime: f64, jobs: u64) -> WorkerCandidate {
        WorkerCandidate {
            peer_id,
            capabilities: WorkerCapabilities {
                gpus: vec![],
                cpu: CpuInfo { model: "test".into(), cores: 4, threads: 8, freq_mhz: 3000 },
                ram_mb: 8192,
                disk_mb: 100_000,
                os: "linux".into(),
                worker_version: "0.1.0".into(),
            },
            uptime_score: uptime,
            jobs_completed: jobs,
            is_busy: false,
        }
    }

    fn cuda_worker(peer_id: PeerId, vram_mb: u64, uptime: f64) -> WorkerCandidate {
        WorkerCandidate {
            peer_id,
            capabilities: WorkerCapabilities {
                gpus: vec![GpuInfo {
                    name: "RTX 4090".into(),
                    vram_mb,
                    backend: GpuBackend::Cuda,
                }],
                cpu: CpuInfo { model: "test".into(), cores: 8, threads: 16, freq_mhz: 3500 },
                ram_mb: 32768,
                disk_mb: 500_000,
                os: "linux".into(),
                worker_version: "0.1.0".into(),
            },
            uptime_score: uptime,
            jobs_completed: 0,
            is_busy: false,
        }
    }

    #[test]
    fn select_workers_cpu_only_returns_top_3() {
        let peers: Vec<PeerId> = (0..5).map(|_| PeerId::random()).collect();
        let candidates = vec![
            cpu_worker(peers[0], 50.0, 10),
            cpu_worker(peers[1], 99.0, 100),
            cpu_worker(peers[2], 80.0, 50),
            cpu_worker(peers[3], 60.0, 20),
            cpu_worker(peers[4], 70.0, 30),
        ];
        let req = MatchRequest {
            required_backend:  GpuBackend::CpuOnly,
            memory_limit_mb:   0,
            max_duration_secs: 3600,
        };
        let selected = select_workers(&candidates, &req);
        assert_eq!(selected.len(), 3);
        // Highest uptime+jobs should be selected first.
        assert_eq!(selected[0], peers[1]);
    }

    #[test]
    fn select_workers_filters_busy() {
        let peer = PeerId::random();
        let mut candidate = cpu_worker(peer, 100.0, 0);
        candidate.is_busy = true;
        let req = MatchRequest {
            required_backend:  GpuBackend::CpuOnly,
            memory_limit_mb:   0,
            max_duration_secs: 3600,
        };
        let selected = select_workers(&[candidate], &req);
        assert!(selected.is_empty());
    }

    #[test]
    fn select_workers_filters_insufficient_vram() {
        let peer = PeerId::random();
        let candidate = cuda_worker(peer, 4096, 100.0);
        let req = MatchRequest {
            required_backend:  GpuBackend::Cuda,
            memory_limit_mb:   8192, // Needs 8 GB but worker only has 4 GB
            max_duration_secs: 3600,
        };
        let selected = select_workers(&[candidate], &req);
        assert!(selected.is_empty());
    }

    #[test]
    fn select_workers_cuda_job_needs_cuda_worker() {
        let cpu_peer  = PeerId::random();
        let cuda_peer = PeerId::random();
        let candidates = vec![
            cpu_worker(cpu_peer, 100.0, 999),
            cuda_worker(cuda_peer, 24576, 80.0),
        ];
        let req = MatchRequest {
            required_backend:  GpuBackend::Cuda,
            memory_limit_mb:   8192,
            max_duration_secs: 3600,
        };
        let selected = select_workers(&candidates, &req);
        assert_eq!(selected, vec![cuda_peer]);
    }
}
