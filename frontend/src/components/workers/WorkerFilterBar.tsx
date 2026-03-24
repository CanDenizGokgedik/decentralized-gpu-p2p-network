'use client'

import { Filter } from 'lucide-react'
import { cn, formatMB } from '@/lib/utils'
import { BACKEND_OPTIONS } from '@/lib/constants'

export interface WorkerFilters {
  backend:      string
  minVramMb:    number
  onlineOnly:   boolean
}

interface WorkerFilterBarProps {
  filters:    WorkerFilters
  onChange:   (f: WorkerFilters) => void
}

const VRAM_STEPS = [0, 1024, 2048, 4096, 8192, 16384, 24576, 40960]

export function WorkerFilterBar({ filters, onChange }: WorkerFilterBarProps) {
  const set = <K extends keyof WorkerFilters>(key: K, val: WorkerFilters[K]) =>
    onChange({ ...filters, [key]: val })

  return (
    <div className="flex flex-wrap items-center gap-4 rounded-xl border border-slate-700 bg-slate-800/40 px-4 py-3">
      <div className="flex items-center gap-2 text-slate-400 shrink-0">
        <Filter className="h-4 w-4" />
        <span className="text-sm font-medium">Filtrele</span>
      </div>

      {/* Backend */}
      <div className="flex items-center gap-2">
        <label className="text-xs text-slate-500 shrink-0">Backend</label>
        <select
          value={filters.backend}
          onChange={e => set('backend', e.target.value)}
          className="rounded-lg border border-slate-700 bg-slate-900 px-3 py-1.5 text-sm text-slate-200 focus:outline-none focus:ring-2 focus:ring-indigo-500"
        >
          <option value="">Tümü</option>
          {BACKEND_OPTIONS.map(opt => (
            <option key={opt.value} value={opt.value}>{opt.label}</option>
          ))}
        </select>
      </div>

      {/* Min VRAM */}
      <div className="flex items-center gap-2">
        <label className="text-xs text-slate-500 shrink-0">Min VRAM</label>
        <select
          value={filters.minVramMb}
          onChange={e => set('minVramMb', +e.target.value)}
          className="rounded-lg border border-slate-700 bg-slate-900 px-3 py-1.5 text-sm text-slate-200 focus:outline-none focus:ring-2 focus:ring-indigo-500"
        >
          {VRAM_STEPS.map(mb => (
            <option key={mb} value={mb}>{mb === 0 ? 'Tümü' : formatMB(mb)}</option>
          ))}
        </select>
      </div>

      {/* Online only toggle */}
      <label className="flex items-center gap-2 cursor-pointer ml-auto">
        <span className="text-sm text-slate-400">Yalnızca Çevrimiçi</span>
        <button
          role="switch"
          aria-checked={filters.onlineOnly}
          onClick={() => set('onlineOnly', !filters.onlineOnly)}
          className={cn(
            'relative inline-flex h-5 w-9 shrink-0 rounded-full border-2 border-transparent transition-colors focus:outline-none focus:ring-2 focus:ring-indigo-500 focus:ring-offset-2 focus:ring-offset-slate-900',
            filters.onlineOnly ? 'bg-indigo-600' : 'bg-slate-700'
          )}
        >
          <span className={cn(
            'pointer-events-none inline-block h-4 w-4 rounded-full bg-white shadow transform transition-transform',
            filters.onlineOnly ? 'translate-x-4' : 'translate-x-0'
          )} />
        </button>
      </label>
    </div>
  )
}
