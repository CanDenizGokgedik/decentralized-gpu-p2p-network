// ── Auth ──────────────────────────────────────────────────────────────────────

export interface RegisterRequest {
  email: string
  password: string
  role: 'hirer' | 'worker' | 'both'
}

export interface LoginRequest {
  email: string
  password: string
}

export interface AuthResponse {
  token: string
  user_id: string
  email: string
  role: string
  expires_at: string
  cu_balance?: number
}

export interface MeResponse {
  user_id: string
  email: string
  role: string
  created_at: string
  cu_balance?: number
}

export interface UserClaims {
  sub: string
  email: string
  role: string
  exp: number
}

// ── Jobs ──────────────────────────────────────────────────────────────────────

export type JobStatus = 'pending' | 'assigned' | 'running' | 'completed' | 'failed' | 'cancelled'
export type GpuBackend = 'cpu_only' | 'cuda' | 'metal' | 'rocm'

export interface Job {
  id: string
  status: JobStatus
  gpu_backend: GpuBackend
  memory_limit_mb?: number
  max_duration_secs?: number
  cu_price?: number
  created_at: string
  assigned_at?: string
  started_at?: string
  finished_at?: string
  error_message?: string
  result_path?: string
  worker_peer_id?: string
}

export interface JobListParams {
  status?: JobStatus
  limit?: number
  offset?: number
}

export interface JobListResponse {
  jobs?: Job[]
  total?: number
}

export interface JobCreateResponse {
  job_id: string
}

// ── Workers ───────────────────────────────────────────────────────────────────

export interface GpuInfo {
  name: string
  vram_mb: number
  backend: string
}

export interface CpuInfo {
  model: string
  cores: number
  threads: number
  freq_mhz: number
}

export interface WorkerCapabilities {
  gpus: GpuInfo[]
  cpu: CpuInfo
  ram_mb: number
  disk_mb: number
  os: string
  worker_version: string
}

export interface WorkerRow {
  peer_id: string
  /** Not returned by API; derived from capabilities.gpus[0].backend on the frontend. */
  gpu_backend?: string
  is_online: boolean
  is_busy?: boolean
  uptime_score: number
  jobs_completed: number
  capabilities: WorkerCapabilities
  last_seen?: string
}

export interface WorkerListParams {
  backend?: string
  min_vram_mb?: number
  online_only?: boolean
  limit?: number
  offset?: number
}

export type WorkerListResponse = WorkerRow[]

// ── Compute Units ─────────────────────────────────────────────────────────────

export interface BalanceResponse {
  cu_balance: number
  cu_reserved: number
  cu_available: number
  unit: string
  recent_transactions?: Transaction[]
}

export interface Transaction {
  id: string
  amount: number
  cu_amount: number
  tx_type: string
  job_id?: string
  description?: string
  created_at: string
}

export interface TxParams {
  limit?: number
  offset?: number
  tx_type?: string
}

export interface TxResponse {
  transactions: Transaction[]
  total: number
  limit: number
  offset: number
}

export interface PricingResponse {
  base_rate_per_hour: number
  multipliers: Record<string, number>
  example_prices_1h: Record<string, number>
  unit: string
}

export interface AllocateRequest {
  user_id: string
  amount: number
  description?: string
}

// ── Admin ─────────────────────────────────────────────────────────────────────

export interface AdminStats {
  users_total: number
  workers_online: number
  jobs_running: number
  jobs_today: number
  jobs_total: number
  cu_allocated_total: number
  cu_consumed_total: number
  jobs_by_day: Array<{ day: string; count: number; completed: number; failed: number }>
  jobs_by_backend: Array<{ backend: string; count: number }>
  // legacy fields for backward compatibility
  total_users?: number
  total_workers?: number
  total_jobs?: number
  cu_allocated?: number
  cu_consumed?: number
}

export interface AdminUser {
  id: string
  email: string
  role: string
  created_at: string
  cu_balance?: number
  cu_reserved?: number
}

export interface UserParams {
  search?: string
  role?: string
  limit?: number
  offset?: number
}

export interface UserListResponse {
  users: AdminUser[]
  total: number
  limit: number
  offset: number
}

export interface UserDetail extends AdminUser {
  jobs?: Job[]
  transactions?: Transaction[]
}

// ── Downloads ─────────────────────────────────────────────────────────────────

export interface PlatformInfo {
  available: boolean
  size_bytes?: number
  size_mb?: number | null
  url?: string
  download_url?: string
}

export interface DownloadInfo {
  platforms: Record<string, PlatformInfo>
}
