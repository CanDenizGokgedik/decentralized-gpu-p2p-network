'use client'

import { useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import Link from 'next/link'
import { Plus, RefreshCw } from 'lucide-react'
import { jobsApi } from '@/lib/api'
import { Button } from '@/components/ui/Button'
import { Card } from '@/components/ui/Card'
import { JobStatusBadge } from '@/components/jobs/JobStatusBadge'
import { PageSpinner } from '@/components/ui/Spinner'
import { formatDate, backendLabel, formatCU, shorten } from '@/lib/utils'
import type { JobStatus } from '@/types'

const STATUS_FILTERS = [
  { value: '', label: 'Tümü' },
  { value: 'pending',   label: 'Bekliyor' },
  { value: 'running',   label: 'Çalışıyor' },
  { value: 'completed', label: 'Tamamlandı' },
  { value: 'failed',    label: 'Başarısız' },
  { value: 'cancelled', label: 'İptal Edildi' },
]

export default function IslerPage() {
  const [statusFilter, setStatusFilter] = useState('')

  const { data: jobs, isLoading, refetch, isFetching } = useQuery({
    queryKey: ['jobs', statusFilter],
    queryFn: () => jobsApi.list({ status: (statusFilter as JobStatus) || undefined, limit: 100 }).then(r => r.data),
    refetchInterval: 15_000,
  })

  const list = Array.isArray(jobs) ? jobs : []

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-slate-100">İşlerim</h1>
          <p className="text-slate-400 mt-1">Gönderdiğiniz tüm GPU işleri</p>
        </div>
        <div className="flex gap-2">
          <Button variant="ghost" size="sm" onClick={() => refetch()} loading={isFetching}>
            <RefreshCw className="h-4 w-4" />
          </Button>
          <Link href="/jobs/new">
            <Button size="sm">
              <Plus className="h-4 w-4" />
              Yeni İş
            </Button>
          </Link>
        </div>
      </div>

      {/* Status filter */}
      <div className="flex gap-2 flex-wrap">
        {STATUS_FILTERS.map(f => (
          <button
            key={f.value}
            onClick={() => setStatusFilter(f.value)}
            className={`rounded-full px-3 py-1.5 text-sm font-medium transition-colors ${
              statusFilter === f.value
                ? 'bg-indigo-600 text-white'
                : 'bg-slate-800 text-slate-400 hover:bg-slate-700 hover:text-slate-200'
            }`}
          >
            {f.label}
          </button>
        ))}
      </div>

      {isLoading ? <PageSpinner /> : list.length === 0 ? (
        <Card className="py-16 text-center">
          <p className="text-slate-400 mb-4">Henüz iş bulunmuyor.</p>
          <Link href="/jobs/new">
            <Button><Plus className="h-4 w-4" />İlk İşini Oluştur</Button>
          </Link>
        </Card>
      ) : (
        <Card className="p-0 overflow-hidden">
          <table className="w-full text-sm">
            <thead className="border-b border-slate-700">
              <tr>
                {['İş ID', 'Durum', 'Backend', 'CU Maliyeti', 'Oluşturulma', ''].map(h => (
                  <th key={h} className="px-4 py-3 text-left font-medium text-slate-400">{h}</th>
                ))}
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-700/50">
              {list.map(job => (
                <tr key={job.id} className="hover:bg-slate-800/50 transition-colors">
                  <td className="px-4 py-3 font-mono text-slate-300">{shorten(job.id, 8)}</td>
                  <td className="px-4 py-3"><JobStatusBadge status={job.status as JobStatus} /></td>
                  <td className="px-4 py-3 text-slate-400">{backendLabel[job.gpu_backend] ?? job.gpu_backend}</td>
                  <td className="px-4 py-3 text-indigo-300">{job.cu_price ? formatCU(job.cu_price) : '—'}</td>
                  <td className="px-4 py-3 text-slate-500">{formatDate(job.created_at)}</td>
                  <td className="px-4 py-3">
                    <Link href={`/jobs/${job.id}`} className="text-indigo-400 hover:text-indigo-300 font-medium">
                      Detay →
                    </Link>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </Card>
      )}
    </div>
  )
}
