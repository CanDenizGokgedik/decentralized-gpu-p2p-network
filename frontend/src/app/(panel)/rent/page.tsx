'use client'

import { useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { RefreshCw } from 'lucide-react'
import { workersApi } from '@/lib/api'
import { Button } from '@/components/ui/Button'
import { PageSpinner } from '@/components/ui/Spinner'
import { WorkerCard } from '@/components/workers/WorkerCard'
import { WorkerFilterBar, type WorkerFilters } from '@/components/workers/WorkerFilterBar'

const DEFAULT_FILTERS: WorkerFilters = {
  backend:    '',
  minVramMb:  0,
  onlineOnly: true,
}

export default function GpuKiralaPage() {
  const [filters, setFilters] = useState<WorkerFilters>(DEFAULT_FILTERS)

  const { data: workers, isLoading, refetch, isFetching } = useQuery({
    queryKey: ['workers', filters],
    queryFn: () => workersApi.list({
      backend:      filters.backend   || undefined,
      min_vram_mb:  filters.minVramMb || undefined,
      online_only:  filters.onlineOnly,
      limit:        100,
    }).then(r => r.data),
    refetchInterval: 30_000,
  })

  const list = Array.isArray(workers) ? workers : []

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-slate-100">GPU Kirala</h1>
          <p className="text-slate-400 mt-1">Ağdaki mevcut GPU worker&apos;larına göz atın</p>
        </div>
        <Button variant="ghost" size="sm" onClick={() => refetch()} loading={isFetching}>
          <RefreshCw className="h-4 w-4" />
        </Button>
      </div>

      <WorkerFilterBar filters={filters} onChange={setFilters} />

      {isLoading ? (
        <PageSpinner />
      ) : list.length === 0 ? (
        <div className="rounded-xl border border-slate-700 bg-slate-800/30 py-20 text-center">
          <p className="text-slate-400">
            {filters.onlineOnly
              ? 'Şu an çevrimiçi worker bulunmuyor.'
              : 'Filtrelere uyan worker bulunamadı.'}
          </p>
          {filters.onlineOnly && (
            <button
              onClick={() => setFilters(f => ({ ...f, onlineOnly: false }))}
              className="mt-3 text-sm text-indigo-400 hover:text-indigo-300 underline"
            >
              Tüm worker&apos;ları göster
            </button>
          )}
        </div>
      ) : (
        <>
          <p className="text-sm text-slate-500">{list.length} worker bulundu</p>
          <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">
            {list.map(w => (
              <WorkerCard key={w.peer_id} worker={w} />
            ))}
          </div>
        </>
      )}
    </div>
  )
}
