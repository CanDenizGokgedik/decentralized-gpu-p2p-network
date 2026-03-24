//! API-layer integration tests for the DecentGPU master node.
//!
//! Tests that do not need a live database run unconditionally.
//! Tests that require a running PostgreSQL instance are marked `#[ignore]`
//! and can be run with:
//!
//!   DATABASE_URL=postgres://... cargo test --test api_integration -- --ignored

// ── Auth validation (no DB required) ─────────────────────────────────────────

mod auth_validation {
    use decentgpu_master::api::auth::{validate_email, validate_password};

    #[test]
    fn valid_emails_are_accepted() {
        assert!(validate_email("user@example.com"));
        assert!(validate_email("a@b.co"));
        assert!(validate_email("user+tag@mail.domain.org"));
        assert!(validate_email("first.last@sub.example.io"));
    }

    #[test]
    fn invalid_emails_are_rejected() {
        assert!(!validate_email(""));
        assert!(!validate_email("notanemail"));
        assert!(!validate_email("@example.com"));    // empty local part
        assert!(!validate_email("user@"));            // empty domain
        assert!(!validate_email("user@nodot"));       // domain without dot
        assert!(!validate_email("user@.start.dot")); // domain starts with dot
    }

    #[test]
    fn strong_passwords_are_accepted() {
        assert!(validate_password("Passw0rd"));
        assert!(validate_password("MyStr0ng!"));
        assert!(validate_password("Abc12345"));
        assert!(validate_password("UPPER lower 1"));
    }

    #[test]
    fn weak_passwords_are_rejected() {
        assert!(!validate_password("short1A"));       // only 7 chars
        assert!(!validate_password("alllower1"));     // no uppercase
        assert!(!validate_password("ALLUPPER1"));     // no lowercase
        assert!(!validate_password("NoDigitHere"));   // no digit
        assert!(!validate_password("Ab1"));            // too short
        assert!(!validate_password(""));
    }
}

// ── Pricing logic (no DB required) ────────────────────────────────────────────

mod pricing {
    use decentgpu_common::types::GpuBackend;
    use decentgpu_master::credits::ComputeUnitLedger;

    #[test]
    fn multipliers_are_correct() {
        let cpu   = ComputeUnitLedger::calculate_price(GpuBackend::CpuOnly, 1.0);
        let metal = ComputeUnitLedger::calculate_price(GpuBackend::Metal,   1.0);
        let rocm  = ComputeUnitLedger::calculate_price(GpuBackend::Rocm,    1.0);
        let cuda  = ComputeUnitLedger::calculate_price(GpuBackend::Cuda,    1.0);

        assert!(cpu > 0,          "base price must be positive");
        assert_eq!(metal, cpu * 3, "Metal multiplier is 3×");
        assert_eq!(rocm,  cpu * 4, "ROCm multiplier is 4×");
        assert_eq!(cuda,  cpu * 5, "CUDA multiplier is 5×");
    }

    #[test]
    fn price_scales_linearly_with_duration() {
        let p1h = ComputeUnitLedger::calculate_price(GpuBackend::Cuda, 1.0);
        let p3h = ComputeUnitLedger::calculate_price(GpuBackend::Cuda, 3.0);
        assert_eq!(p3h, p1h * 3, "price must scale linearly with duration");
    }

    #[test]
    fn all_backends_are_positive() {
        for backend in [GpuBackend::CpuOnly, GpuBackend::Metal, GpuBackend::Rocm, GpuBackend::Cuda] {
            assert!(
                ComputeUnitLedger::calculate_price(backend, 1.0) > 0,
                "price for {backend:?} must be positive"
            );
        }
    }
}

// ── HTTP tests (require DATABASE_URL) ─────────────────────────────────────────
//
// These tests are `#[ignore]`d by default. Run with:
//   DATABASE_URL=postgres://... cargo test --test api_integration -- --ignored

mod http {
    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn test_register_and_login() {
        // Spin up AppState → build router → send POST /api/v1/auth/register,
        // verify 201 + token, then POST /api/v1/auth/login, verify 200 + token.
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn test_job_submit_insufficient_cu() {
        // Register user (no CU allocation), POST /api/v1/jobs,
        // verify 400 "insufficient compute units".
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn test_admin_allocate_and_job_submit() {
        // Bootstrap admin, allocate CU to hirer, submit job, verify 201.
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn test_worker_list_filters() {
        // Register workers with different backends, test backend= / online_only= filters.
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn test_download_info() {
        // GET /api/v1/downloads/info — verify JSON structure contains "platforms" key.
    }
}
