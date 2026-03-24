'use client'

import { useQuery } from '@tanstack/react-query'
import { useAuth } from '@/lib/auth'
import { workersApi, jobsApi } from '@/lib/api'
import { Card, CardHeader, CardTitle } from '@/components/ui/Card'
import { Badge } from '@/components/ui/Badge'
import { JobStatusBadge } from '@/components/jobs/JobStatusBadge'
import { PageSpinner } from '@/components/ui/Spinner'
import { formatDate, formatMB, shorten, backendLabel } from '@/lib/utils'
import { Cpu, Activity, CheckCircle, Clock, Wifi, WifiOff } from 'lucide-react'
import type { JobStatus } from '@/types'

export default function IsciPaneliPage() {
  const { user } = useAuth()

  const { data: myWorker, isLoading: workerLoading } = useQuery({
    queryKey: ['my-worker'],
    queryFn:  () => workersApi.me().then(r => r.data),
    enabled:  !!user,
    retry:    false,
  })

  const { data: jobs, isLoading: jobsLoading } = useQuery({
    queryKey: ['jobs'],
    queryFn:  () => jobsApi.list({ limit: 50 }).then(r => r.data),
    enabled:  !!user,
    refetchInterval: 10_000,
  })

  const isLoading = workerLoading || jobsLoading

  if (isLoading) return <PageSpinner />

  const jobList = Array.isArray(jobs) ? jobs : []
  const running   = jobList.filter(j => j.status === 'running').length
  const completed = jobList.filter(j => j.status === 'completed').length

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-bold text-slate-100">Worker Paneli</h1>
        <p className="text-slate-400 mt-1">GPU&apos;nuzun durumu ve iş geçmişi</p>
      </div>

      {/* Worker status */}
      {!myWorker ? (
        <Card className="border-amber-700/40 bg-amber-950/10">
          <div className="flex items-start gap-3">
            <WifiOff className="h-5 w-5 text-amber-400 shrink-0 mt-0.5" />
            <div>
              <p className="font-semibold text-amber-300">Worker Bağlı Değil</p>
              <p className="text-sm text-amber-400/70 mt-1">
                GPU&apos;nuzu ağa bağlamak için{' '}
                <a href="/account/client" className="underline font-medium">istemci yazılımını</a>{' '}
                indirin ve çalıştırın.
              </p>
            </div>
          </div>
        </Card>
      ) : (
        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
          <Card className="flex items-center gap-4">
            <div className={`flex h-12 w-12 shrink-0 items-center justify-center rounded-xl ${
              myWorker.is_online ? 'bg-emerald-500/10' : 'bg-slate-700/30'
            }`}>
              {myWorker.is_online
                ? <Wifi className="h-6 w-6 text-emerald-400" />
                : <WifiOff className="h-6 w-6 text-slate-500" />
              }
            </div>
            <div>
              <p className="text-sm text-slate-400">Bağlantı</p>
              <p className="font-bold text-slate-100">
                {myWorker.is_online ? 'Çevrimiçi' : 'Çevrimdışı'}
              </p>
            </div>
          </Card>

          <Card className="flex items-center gap-4">
            <div className="flex h-12 w-12 shrink-0 items-center justify-center rounded-xl bg-indigo-500/10">
              <Cpu className="h-6 w-6 text-indigo-400" />
            </div>
            <div>
              <p className="text-sm text-slate-400">GPU</p>
              <p className="font-bold text-slate-100 text-sm">
                {myWorker.capabilities?.gpus?.[0]?.name ?? 'CPU-Only'}
              </p>
            </div>
          </Card>

          <Card className="flex items-center gap-4">
            <div className="flex h-12 w-12 shrink-0 items-center justify-center rounded-xl bg-blue-500/10">
              <Activity className="h-6 w-6 text-blue-400" />
            </div>
            <div>
              <p className="text-sm text-slate-400">Aktif İş</p>
              <p className="text-2xl font-bold text-slate-100">{running}</p>
            </div>
          </Card>

          <Card className="flex items-center gap-4">
            <div className="flex h-12 w-12 shrink-0 items-center justify-center rounded-xl bg-emerald-500/10">
              <CheckCircle className="h-6 w-6 text-emerald-400" />
            </div>
            <div>
              <p className="text-sm text-slate-400">Tamamlanan</p>
              <p className="text-2xl font-bold text-slate-100">{myWorker.jobs_completed}</p>
            </div>
          </Card>
        </div>
      )}

      {/* Worker details */}
      {myWorker && (
        <Card>
          <CardHeader><CardTitle>Worker Bilgileri</CardTitle></CardHeader>
          <div className="grid sm:grid-cols-2 gap-4 text-sm">
            <div className="space-y-3">
              <div className="flex justify-between">
                <span className="text-slate-400">Peer ID</span>
                <span className="font-mono text-xs text-slate-300">{shorten(myWorker.peer_id, 16)}</span>
              </div>
              <div className="flex justify-between">
                <span className="text-slate-400">Backend</span>
                <Badge variant="info">{(() => { const b = myWorker.capabilities?.gpus?.[0]?.backend ?? myWorker.gpu_backend ?? 'cpu_only'; return backendLabel[b] ?? b })()}</Badge>
              </div>
              <div className="flex justify-between">
                <span className="text-slate-400">VRAM</span>
                <span className="text-slate-200">
                  {myWorker.capabilities?.gpus?.[0]?.vram_mb
                    ? formatMB(myWorker.capabilities.gpus[0].vram_mb)
                    : '—'}
                </span>
              </div>
            </div>
            <div className="space-y-3">
              <div className="flex justify-between">
                <span className="text-slate-400">Uptime Skoru</span>
                <span className="text-emerald-400 font-semibold">{myWorker.uptime_score.toFixed(1)}%</span>
              </div>
              <div className="flex justify-between">
                <span className="text-slate-400">Toplam İş</span>
                <span className="text-slate-200">{myWorker.jobs_completed}</span>
              </div>
              <div className="flex justify-between">
                <span className="text-slate-400">Son Görülme</span>
                <span className="text-slate-400 text-xs">
                  {myWorker.last_seen ? formatDate(myWorker.last_seen) : '—'}
                </span>
              </div>
            </div>
          </div>
        </Card>
      )}

      {/* Recent jobs */}
      <Card>
        <CardHeader>
          <CardTitle>İşler ({jobList.length})</CardTitle>
        </CardHeader>
        {jobList.length === 0 ? (
          <p className="py-8 text-center text-sm text-slate-500">Henüz iş yok.</p>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-slate-700">
                  {['İş ID', 'Durum', 'Backend', 'Oluşturulma'].map(h => (
                    <th key={h} className="px-4 py-3 text-left font-medium text-slate-400">{h}</th>
                  ))}
                </tr>
              </thead>
              <tbody className="divide-y divide-slate-700/50">
                {jobList.map(job => (
                  <tr key={job.id} className="hover:bg-slate-800/30">
                    <td className="px-4 py-3 font-mono text-slate-300">{shorten(job.id, 8)}</td>
                    <td className="px-4 py-3"><JobStatusBadge status={job.status as JobStatus} /></td>
                    <td className="px-4 py-3 text-slate-400">{backendLabel[job.gpu_backend] ?? job.gpu_backend}</td>
                    <td className="px-4 py-3 text-slate-500">{formatDate(job.created_at)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </Card>
    </div>
  )
}
