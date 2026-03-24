//! Integration tests for the DecentGPU master node.
//!
//! These tests exercise the scheduler and transfer utilities entirely in-process
//! (no real Docker, no real P2P connections, no database required).

// ── Scheduler integration tests ───────────────────────────────────────────────

#[cfg(test)]
mod scheduler_tests {
    use decentgpu_master::scheduler::matcher::{MatchRequest, WorkerCandidate};
    use decentgpu_master::scheduler::matcher::select_workers;
    use decentgpu_common::types::{CpuInfo, GpuBackend, GpuInfo, WorkerCapabilities};
    use libp2p::PeerId;

    fn make_candidate(
        peer_id: PeerId,
        backend: GpuBackend,
        vram_mb: u64,
        uptime: f64,
        jobs: u64,
        busy: bool,
    ) -> WorkerCandidate {
        WorkerCandidate {
            peer_id,
            capabilities: WorkerCapabilities {
                gpus: if backend == GpuBackend::CpuOnly {
                    vec![]
                } else {
                    vec![GpuInfo { name: "GPU".into(), vram_mb, backend }]
                },
                cpu: CpuInfo {
                    model: "Test CPU".into(),
                    cores: 4,
                    threads: 8,
                    freq_mhz: 3000,
                },
                ram_mb: 16_384,
                disk_mb: 100_000,
                os: "linux".into(),
                worker_version: "0.1.0".into(),
            },
            uptime_score: uptime,
            jobs_completed: jobs,
            is_busy: busy,
        }
    }

    #[test]
    fn scheduler_selects_best_cuda_worker() {
        let peer_a = PeerId::random();
        let peer_b = PeerId::random();
        let peer_c = PeerId::random();

        let candidates = vec![
            make_candidate(peer_a, GpuBackend::Cuda, 8_192, 90.0, 100, false),
            make_candidate(peer_b, GpuBackend::Cuda, 16_384, 80.0, 50, false),
            make_candidate(peer_c, GpuBackend::CpuOnly, 0, 95.0, 200, false),
        ];

        let request = MatchRequest {
            required_backend: GpuBackend::Cuda,
            memory_limit_mb: 4_096,
            max_duration_secs: 3600,
        };

        let selected = select_workers(&candidates, &request);

        // Only CUDA workers should be selected; CPU-only must be excluded.
        assert!(!selected.is_empty());
        assert!(selected.iter().all(|p| *p == peer_a || *p == peer_b));
        assert!(!selected.contains(&peer_c));
    }

    #[test]
    fn scheduler_excludes_busy_workers() {
        let peer_a = PeerId::random();
        let peer_b = PeerId::random();

        let candidates = vec![
            make_candidate(peer_a, GpuBackend::CpuOnly, 0, 100.0, 10, true),  // busy
            make_candidate(peer_b, GpuBackend::CpuOnly, 0, 80.0, 5, false),   // available
        ];

        let request = MatchRequest {
            required_backend: GpuBackend::CpuOnly,
            memory_limit_mb: 512,
            max_duration_secs: 3600,
        };

        let selected = select_workers(&candidates, &request);
        assert_eq!(selected, vec![peer_b]);
    }

    #[test]
    fn scheduler_returns_empty_when_no_workers() {
        let request = MatchRequest {
            required_backend: GpuBackend::Cuda,
            memory_limit_mb: 4_096,
            max_duration_secs: 3600,
        };
        let selected = select_workers(&[], &request);
        assert!(selected.is_empty());
    }

    #[test]
    fn scheduler_caps_selection_at_3() {
        let candidates: Vec<WorkerCandidate> = (0..10)
            .map(|i| make_candidate(PeerId::random(), GpuBackend::CpuOnly, 0, i as f64 * 10.0, i as u64 * 5, false))
            .collect();

        let request = MatchRequest {
            required_backend: GpuBackend::CpuOnly,
            memory_limit_mb: 512,
            max_duration_secs: 3600,
        };

        let selected = select_workers(&candidates, &request);
        assert!(selected.len() <= 3);
    }
}

// ── Transfer protocol tests ───────────────────────────────────────────────────

#[cfg(test)]
mod transfer_tests {
    use decentgpu_common::transfer::{receive_file_chunked, send_file_chunked};
    use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

    /// Helper: create an in-memory bidirectional pipe compatible with futures IO.
    macro_rules! duplex_pair {
        () => {{
            let (client, server) = tokio::io::duplex(256 * 1024);
            (
                tokio_util::compat::TokioAsyncReadCompatExt::compat(client),
                tokio_util::compat::TokioAsyncReadCompatExt::compat(server),
            )
        }};
    }

    #[tokio::test]
    async fn transfer_roundtrip_small_file() {
        // Write a temp file.
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("input.tar");
        let dst = dir.path().join("output.tar");

        let payload = b"hello decentgpu world!";
        tokio::fs::write(&src, payload).await.unwrap();

        // Open a duplex pipe.
        let (client_io, server_io) = tokio::io::duplex(1024 * 1024);
        let mut sender_io = TokioAsyncReadCompatExt::compat(client_io);
        let mut receiver_io = TokioAsyncReadCompatExt::compat(server_io);

        // Run sender and receiver concurrently.
        let src_clone = src.clone();
        let dst_clone = dst.clone();

        let (send_result, recv_result) = tokio::join!(
            async { send_file_chunked(&mut sender_io, &src_clone, "test-transfer-id").await },
            async { receive_file_chunked(&mut receiver_io, &dst_clone).await },
        );

        let sent_bytes = send_result.unwrap();
        let recv_bytes = recv_result.unwrap();

        assert_eq!(sent_bytes, recv_bytes);
        assert_eq!(sent_bytes, payload.len() as u64);

        let received = tokio::fs::read(&dst).await.unwrap();
        assert_eq!(received, payload);
    }

    #[tokio::test]
    async fn transfer_roundtrip_large_file() {
        // 200 KiB payload — spans multiple 64 KiB chunks.
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("large.bin");
        let dst = dir.path().join("large_out.bin");

        let payload: Vec<u8> = (0u8..=255).cycle().take(200 * 1024).collect();
        tokio::fs::write(&src, &payload).await.unwrap();

        let (client_io, server_io) = tokio::io::duplex(4 * 1024 * 1024);
        let mut sender_io = TokioAsyncReadCompatExt::compat(client_io);
        let mut receiver_io = TokioAsyncReadCompatExt::compat(server_io);

        let src_clone = src.clone();
        let dst_clone = dst.clone();

        let (send_result, recv_result) = tokio::join!(
            async { send_file_chunked(&mut sender_io, &src_clone, "large-xfer").await },
            async { receive_file_chunked(&mut receiver_io, &dst_clone).await },
        );

        assert_eq!(send_result.unwrap(), payload.len() as u64);
        assert_eq!(recv_result.unwrap(), payload.len() as u64);

        let received = tokio::fs::read(&dst).await.unwrap();
        assert_eq!(received, payload);
    }
}

// ── Compute unit pricing tests ────────────────────────────────────────────────

#[cfg(test)]
mod pricing_tests {
    use decentgpu_common::types::GpuBackend;
    use decentgpu_master::credits::ComputeUnitLedger;

    #[test]
    fn cpu_only_1h_base_rate() {
        let price = ComputeUnitLedger::calculate_price(GpuBackend::CpuOnly, 1.0);
        assert!(price > 0, "price must be positive");
    }

    #[test]
    fn cuda_more_expensive_than_cpu() {
        let cpu_price  = ComputeUnitLedger::calculate_price(GpuBackend::CpuOnly, 1.0);
        let cuda_price = ComputeUnitLedger::calculate_price(GpuBackend::Cuda, 1.0);
        assert!(cuda_price > cpu_price, "CUDA must cost more than CPU-only");
    }

    #[test]
    fn longer_duration_costs_more() {
        let price_1h = ComputeUnitLedger::calculate_price(GpuBackend::Cuda, 1.0);
        let price_2h = ComputeUnitLedger::calculate_price(GpuBackend::Cuda, 2.0);
        assert!(price_2h > price_1h, "2h must cost more than 1h");
    }

    #[test]
    fn price_multipliers_order() {
        let cpu   = ComputeUnitLedger::calculate_price(GpuBackend::CpuOnly, 1.0);
        let metal = ComputeUnitLedger::calculate_price(GpuBackend::Metal, 1.0);
        let rocm  = ComputeUnitLedger::calculate_price(GpuBackend::Rocm, 1.0);
        let cuda  = ComputeUnitLedger::calculate_price(GpuBackend::Cuda, 1.0);
        assert!(cpu < metal);
        assert!(metal < rocm);
        assert!(rocm < cuda);
    }
}
