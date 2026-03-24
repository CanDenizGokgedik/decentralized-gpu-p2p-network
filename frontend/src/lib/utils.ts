import { clsx, type ClassValue } from 'clsx'
import { twMerge } from 'tailwind-merge'
import { format, formatDistanceToNow } from 'date-fns'
import { tr } from 'date-fns/locale'

/** Merge Tailwind classes safely. */
export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

/** Format a CU amount with unit label. */
export function formatCU(amount: number | undefined | null): string {
  if (amount == null) return '— CU'
  return `${amount.toLocaleString('tr-TR')} CU`
}

/** Format bytes to human-readable size. */
export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`
  return `${(bytes / 1024 / 1024 / 1024).toFixed(2)} GB`
}

/** Format MB to human-readable size. */
export function formatMB(mb: number): string {
  if (mb < 1024) return `${mb} MB`
  return `${(mb / 1024).toFixed(1)} GB`
}

/** Format a date string to Turkish locale. */
export function formatDate(dateStr: string | undefined | null): string {
  if (!dateStr) return '—'
  try {
    return format(new Date(dateStr), 'dd MMM yyyy HH:mm', { locale: tr })
  } catch {
    return '—'
  }
}

/** Format a date as relative time ("3 dakika önce"). */
export function formatRelative(dateStr: string | undefined | null): string {
  if (!dateStr) return '—'
  try {
    return formatDistanceToNow(new Date(dateStr), { addSuffix: true, locale: tr })
  } catch {
    return '—'
  }
}

/** Shorten a UUID/peer-id to first N chars. */
export function shorten(id: string | undefined | null, len = 12): string {
  if (!id) return '—'
  return id.length > len ? id.slice(0, len) + '…' : id
}

/** Turkish labels for job status. */
export const statusLabel: Record<string, string> = {
  pending:   'Bekliyor',
  assigned:  'Atandı',
  running:   'Çalışıyor',
  completed: 'Tamamlandı',
  failed:    'Başarısız',
  cancelled: 'İptal Edildi',
}

/** Turkish labels for GPU backend. */
export const backendLabel: Record<string, string> = {
  cpu_only: 'CPU (Temel)',
  cuda:     'NVIDIA CUDA',
  metal:    'Apple Silicon',
  rocm:     'AMD ROCm',
}

/** Turkish labels for transaction types. */
export const txTypeLabel: Record<string, string> = {
  // Backend canonical values
  allocation: 'Yönetici Tahsisi',
  job_debit:  'İş Kesintisi',
  job_credit: 'İş Kazancı',
  job_refund: 'İş İadesi',
  // Frontend filter aliases
  purchase: 'Satın Alma',
  usage:    'Kullanım',
  admin:    'Admin',
  refund:   'İade',
}

/** CU rate per hour per backend. */
export const cuRates: Record<string, number> = {
  cpu_only: 1,
  metal:    3,
  rocm:     4,
  cuda:     5,
}

/** Calculate estimated CU cost. */
export function estimateCU(backend: string, durationSecs: number): number {
  const hours = durationSecs / 3600
  const rate   = cuRates[backend] ?? 1
  return Math.ceil(10 * rate * hours)
}
