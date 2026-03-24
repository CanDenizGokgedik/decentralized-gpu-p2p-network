export const APP_NAME = process.env.NEXT_PUBLIC_APP_NAME ?? 'DecentGPU'

export const API_URL =
  (typeof window === 'undefined'
    ? process.env.NEXT_PUBLIC_API_URL
    : process.env.NEXT_PUBLIC_API_URL) ?? 'http://localhost:8888'

export const BOOTSTRAP_PEER_ID =
  process.env.NEXT_PUBLIC_BOOTSTRAP_PEER_ID ?? '<bootstrap-peer-id>'

export const BOOTSTRAP_ADDR =
  process.env.NEXT_PUBLIC_BOOTSTRAP_ADDR ?? '/ip4/127.0.0.1/tcp/9000'

export const TOKEN_KEY = 'decentgpu_token'
export const USER_KEY  = 'decentgpu_user'

export const PLATFORMS = [
  { key: 'linux-x86_64',   label: 'Linux (x86_64)',   icon: '🐧' },
  { key: 'linux-aarch64',  label: 'Linux (ARM64)',     icon: '🐧' },
  { key: 'darwin-aarch64', label: 'macOS (Apple Silicon)', icon: '🍎' },
  { key: 'darwin-x86_64',  label: 'macOS (Intel)',     icon: '🍎' },
  { key: 'windows-x86_64', label: 'Windows (x86_64)',  icon: '🪟' },
] as const

export const BACKEND_OPTIONS = [
  { value: 'cpu_only', label: 'CPU (Temel)',       rate: 1, description: 'GPU gerekmez' },
  { value: 'cuda',     label: 'NVIDIA CUDA',       rate: 5, description: 'Derin öğrenme için ideal' },
  { value: 'metal',    label: 'Apple Silicon',     rate: 3, description: 'Metal hızlandırma' },
  { value: 'rocm',     label: 'AMD ROCm',          rate: 4, description: 'Açık kaynak GPU' },
] as const

export const ROLE_OPTIONS = [
  { value: 'hirer',  label: 'İş Veren (Hirer)',  description: 'GPU kiralar, model eğitir' },
  { value: 'worker', label: 'İşçi (Worker)',      description: "GPU'sunu kiraya verir" },
  { value: 'both',   label: 'Her İkisi',          description: 'Hem kiralar hem kiraya verir' },
] as const

export const NAV_ITEMS = [
  { href: '/dashboard',        label: 'Genel Bakış',    icon: 'Home',      roles: ['hirer','worker','both','admin'] },
  { href: '/rent',             label: 'GPU Kirala',     icon: 'Cpu',       roles: ['hirer','worker','both','admin'] },
  { href: '/jobs',             label: 'İşlerim',        icon: 'ClipboardList', roles: ['hirer','worker','both','admin'] },
  { href: '/worker-dashboard', label: 'İşçi Paneli',    icon: 'Wrench',    roles: ['worker','both'] },
  { href: '/compute-units',    label: 'Compute Units',  icon: 'Coins',     roles: ['hirer','worker','both','admin'] },
  { href: '/account',          label: 'Hesabım',        icon: 'Settings',  roles: ['hirer','worker','both','admin'] },
  { href: '/account/client',   label: 'İstemci İndir',  icon: 'Download',  roles: ['hirer','worker','both','admin'] },
  { href: '/admin',            label: 'Yönetim',        icon: 'Shield',    roles: ['admin'] },
] as const
