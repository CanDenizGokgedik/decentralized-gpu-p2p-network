import axios from 'axios'
import { getToken, clearToken } from './auth'
import type {
  RegisterRequest, LoginRequest, AuthResponse, MeResponse,
  Job, JobListParams, JobListResponse, JobCreateResponse,
  WorkerRow, WorkerListParams,
  BalanceResponse, TxParams, TxResponse, PricingResponse, AllocateRequest,
  AdminStats, UserParams, UserListResponse, UserDetail,
  DownloadInfo,
} from '@/types'

// WS_BASE is used only for WebSocket and direct download URLs
const WS_BASE = process.env.NEXT_PUBLIC_API_URL ?? 'http://localhost:8888'

export const api = axios.create({
  baseURL: '',
  withCredentials: true,
})

// ── Request interceptor: attach JWT ──────────────────────────────────────────

api.interceptors.request.use(config => {
  const token = getToken()
  if (token) {
    config.headers = config.headers ?? {}
    config.headers.Authorization = `Bearer ${token}`
  }
  return config
})

// ── Response interceptor: handle 401 ─────────────────────────────────────────

api.interceptors.response.use(
  res => res,
  err => {
    if (err.response?.status === 401 && typeof window !== 'undefined') {
      clearToken()
      window.location.href = '/login'
    }
    return Promise.reject(err)
  }
)

// ── Auth ──────────────────────────────────────────────────────────────────────

export const authApi = {
  register: (data: RegisterRequest) =>
    api.post<AuthResponse>('/api/auth/register', data),
  login: (data: LoginRequest) =>
    api.post<AuthResponse>('/api/auth/login', data),
  me: () =>
    api.get<MeResponse>('/api/auth/me'),
}

// ── Jobs ──────────────────────────────────────────────────────────────────────

export const jobsApi = {
  list: (params?: JobListParams) =>
    api.get<JobListResponse>('/api/jobs', { params }),
  create: (form: FormData) =>
    api.post<JobCreateResponse>('/api/jobs', form),
  get: (id: string) =>
    api.get<Job>(`/api/jobs/${id}`),
  cancel: (id: string) =>
    api.post(`/api/jobs/${id}/cancel`),
  download: (id: string) =>
    api.get(`/api/jobs/${id}/result`, { responseType: 'blob' }),
  logsUrl: (id: string, token: string) =>
    `${WS_BASE}/api/jobs/${id}/logs?token=${encodeURIComponent(token)}`,
  terminalUrl: (id: string, token: string) =>
    `${WS_BASE.replace(/^http/, 'ws')}/api/jobs/${id}/terminal?token=${encodeURIComponent(token)}`,
}

// ── Workers ───────────────────────────────────────────────────────────────────

export const workersApi = {
  list: (params?: WorkerListParams) =>
    api.get<WorkerRow[]>('/api/workers', { params }),
  me: () =>
    api.get<WorkerRow>('/api/workers/me'),
  get: (peerId: string) =>
    api.get<WorkerRow>(`/api/workers/${peerId}`),
}

// ── Compute Units ─────────────────────────────────────────────────────────────

export const computeUnitsApi = {
  balance: () =>
    api.get<BalanceResponse>('/api/compute-units/balance'),
  transactions: (params?: TxParams) =>
    api.get<TxResponse>('/api/compute-units/transactions', { params }),
  pricing: () =>
    api.get<PricingResponse>('/api/compute-units/pricing'),
  allocate: (data: AllocateRequest) =>
    api.post('/api/compute-units/allocate', data),
}

// ── Admin ─────────────────────────────────────────────────────────────────────

export const adminApi = {
  stats: () =>
    api.get<AdminStats>('/api/admin/stats'),
  users: (params?: UserParams) =>
    api.get<UserListResponse>('/api/admin/users', { params }),
  getUser: (id: string) =>
    api.get<UserDetail>(`/api/admin/users/${id}`),
  updateRole: (id: string, role: string) =>
    api.patch(`/api/admin/users/${id}/role`, { role }),
  disconnectWorker: (peerId: string) =>
    api.post(`/api/admin/workers/${peerId}/disconnect`),
  uploadWorkerBinary: (platform: string, file: File) => {
    return api.post(`/api/admin/downloads/worker/${platform}`, file, {
      headers: { 'Content-Type': 'application/octet-stream' }
    })
  },
}

// ── Downloads ─────────────────────────────────────────────────────────────────

export const downloadsApi = {
  info: () =>
    api.get<DownloadInfo>('/api/downloads/info'),
  url: (platform: string) =>
    `${WS_BASE}/api/downloads/worker/${platform}`,
  downloadUrl: (platform: string) =>
    `/api/downloads/worker/${platform}`,
}
