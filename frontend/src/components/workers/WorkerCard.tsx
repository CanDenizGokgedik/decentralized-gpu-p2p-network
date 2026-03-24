'use client'

import { Cpu, Zap, CheckCircle, Clock } from 'lucide-react'
import { Card } from '@/components/ui/Card'
import { Badge } from '@/components/ui/Badge'
import { cn, formatMB, shorten } from '@/lib/utils'
import { backendLabel } from '@/lib/utils'
import type { WorkerRow } from '@/types'

interface WorkerCardProps {
  worker: WorkerRow
}

const backendVariant: Record<string, 'default' | 'success' | 'warning' | 'danger' | 'info'> = {
  cuda:   'success',
  rocm:   'info',
  metal:  'warning',
  cpu_only: 'default',
}

export function WorkerCard({ worker }: WorkerCardProps) {
  // Derive gpu_backend from capabilities since the API doesn't return it as a top-level field
  const gpu        = worker.capabilities?.gpus?.[0]
  const gpuBackend = gpu?.backend ?? worker.gpu_backend ?? 'cpu_only'
  const vram       = gpu?.vram_mb

  return (
    <Card className={cn(
      'flex flex-col gap-4 transition-all hover:border-slate-600',
      worker.is_online ? '' : 'opacity-60'
    )}>
      {/* Header */}
      <div className="flex items-start justify-between gap-2">
        <div className="min-w-0">
          <p className="font-mono text-xs text-slate-400 truncate">{shorten(worker.peer_id, 16)}</p>
          <p className="mt-1 font-semibold text-slate-100 truncate">
            {gpu?.name ?? 'CPU-Only Worker'}
          </p>
        </div>
        <span className={cn(
          'flex shrink-0 items-center gap-1.5 rounded-full px-2 py-0.5 text-xs font-medium',
          worker.is_online
            ? 'bg-emerald-900/40 text-emerald-400'
            : 'bg-slate-800 text-slate-500'
        )}>
          <span className={cn(
            'h-1.5 w-1.5 rounded-full',
            worker.is_online ? 'bg-emerald-400 animate-pulse' : 'bg-slate-600'
          )} />
          {worker.is_online ? 'Çevrimiçi' : 'Çevrimdışı'}
        </span>
      </div>

      {/* GPU Info */}
      <div className="grid grid-cols-2 gap-3">
        <div className="rounded-lg bg-slate-800/60 px-3 py-2">
          <p className="text-xs text-slate-500">Backend</p>
          <div className="mt-1">
            <Badge variant={backendVariant[gpuBackend] ?? 'default'}>
              {backendLabel[gpuBackend] ?? gpuBackend}
            </Badge>
          </div>
        </div>
        <div className="rounded-lg bg-slate-800/60 px-3 py-2">
          <p className="text-xs text-slate-500">VRAM</p>
          <p className="mt-1 font-semibold text-sm text-slate-200">
            {vram ? formatMB(vram) : '—'}
          </p>
        </div>
      </div>

      {/* Stats row */}
      <div className="flex items-center justify-between text-xs text-slate-500 border-t border-slate-700 pt-3">
        <span className="flex items-center gap-1">
          <CheckCircle className="h-3.5 w-3.5 text-emerald-500" />
          {worker.jobs_completed} iş tamamlandı
        </span>
        <span className="flex items-center gap-1">
          <Clock className="h-3.5 w-3.5 text-indigo-400" />
          {worker.uptime_score.toFixed(1)}% uptime
        </span>
      </div>

      {/* Extra GPUs */}
      {worker.capabilities?.gpus && worker.capabilities.gpus.length > 1 && (
        <div className="text-xs text-slate-500 flex items-center gap-1">
          <Cpu className="h-3.5 w-3.5" />
          +{worker.capabilities.gpus.length - 1} ek GPU
        </div>
      )}
    </Card>
  )
}
