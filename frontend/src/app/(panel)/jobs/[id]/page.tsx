'use client'

import { use, useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { Download, X, Check, Clock, Cpu, Activity } from 'lucide-react'
import dynamic from 'next/dynamic'
import { jobsApi } from '@/lib/api'
import { getToken } from '@/lib/auth'
import { Button } from '@/components/ui/Button'
import { Card, CardHeader, CardTitle } from '@/components/ui/Card'
import { JobStatusBadge } from '@/components/jobs/JobStatusBadge'
import { PageSpinner } from '@/components/ui/Spinner'
import { useToast } from '@/components/ui/Toast'
import { formatDate, backendLabel, formatMB, formatCU, shorten } from '@/lib/utils'
import type { JobStatus } from '@/types'

// Load Terminal client-only (xterm needs window)
const Terminal = dynamic(() => import('@/components/jobs/Terminal').then(m => m.Terminal), {
  ssr: false,
  loading: () => (
    <div className="flex h-full items-center justify-center bg-[#0d1117] rounded-xl border border-slate-700">
      <p className="text-slate-500 text-sm">Terminal yükleniyor…</p>
    </div>
  ),
})

const TIMELINE_STEPS: { key: string; label: string }[] = [
  { key: 'created',  label: 'Oluşturuldu' },
  { key: 'building', label: 'İmaj Hazırlanıyor' },
  { key: 'assigned', label: 'Atandı' },
  { key: 'running',  label: 'Çalışıyor' },
  { key: 'done',     label: 'Tamamlandı' },
]

function statusToTimelineIndex(status: string) {
  const map: Record<string, number> = {
    pending: 0, assigned: 2, running: 3, completed: 4, failed: 4, cancelled: 4,
  }
  return map[status] ?? 0
}

export default function IsDetayPage({ params }: { params: Promise<{ id: string }> }) {
  const { id }       = use(params)
  const { toast }    = useToast()
  const queryClient  = useQueryClient()
  const [downloading, setDownloading] = useState(false)

  const { data: job, isLoading } = useQuery({
    queryKey: ['job', id],
    queryFn:  () => jobsApi.get(id).then(r => r.data),
    refetchInterval: query => (query.state.data?.status === 'running' ? 5_000 : 30_000),
  })

  const cancelMut = useMutation({
    mutationFn: () => jobsApi.cancel(id),
    onSuccess: () => {
      toast('İş iptal edildi.', 'info')
      queryClient.invalidateQueries({ queryKey: ['job', id] })
      queryClient.invalidateQueries({ queryKey: ['jobs'] })
    },
    onError: () => toast('İptal başarısız.', 'error'),
  })

  const downloadResult = async () => {
    const token = getToken()
    try {
      setDownloading(true)
      const response = await fetch(`/api/jobs/${id}/result`, {
        headers: { Authorization: `Bearer ${token}` }
      })
      if (!response.ok) {
        toast('Sonuç dosyası bulunamadı', 'error')
        return
      }
      const disposition = response.headers.get('content-disposition') ?? ''
      const match = disposition.match(/filename="([^"]+)"/)
      const filename = match?.[1] ?? `result-${id}.tar.gz`
      const blob = await response.blob()
      const url = URL.createObjectURL(blob)
      const a = document.createElement('a')
      a.href = url
      a.download = filename
      document.body.appendChild(a)
      a.click()
      document.body.removeChild(a)
      URL.revokeObjectURL(url)
    } catch {
      toast('İndirme başarısız.', 'error')
    } finally {
      setDownloading(false)
    }
  }

  if (isLoading) return <PageSpinner />
  if (!job) return <div className="text-slate-400">İş bulunamadı.</div>

  const timelineIdx = statusToTimelineIndex(job.status)
  const canCancel   = ['pending', 'assigned'].includes(job.status)

  return (
    <div className="space-y-6">
      <div className="flex items-start justify-between gap-4">
        <div>
          <h1 className="text-xl font-bold text-slate-100">
            İş Detayı — <span className="font-mono text-slate-400 text-lg">{shorten(job.id, 12)}</span>
          </h1>
          <div className="mt-2 flex items-center gap-3">
            <JobStatusBadge status={job.status as JobStatus} />
          </div>
        </div>
        <div className="flex gap-2">
          {job.status === 'completed' && (
            <Button size="sm" variant="secondary" onClick={downloadResult} loading={downloading}>
              <Download className="h-4 w-4" />
              {downloading ? 'İndiriliyor...' : 'Sonucu İndir'}
            </Button>
          )}
          {canCancel && (
            <Button size="sm" variant="danger" loading={cancelMut.isPending} onClick={() => cancelMut.mutate()}>
              <X className="h-4 w-4" />
              İptal Et
            </Button>
          )}
        </div>
      </div>

      <div className="grid gap-6 lg:grid-cols-2 xl:grid-cols-[400px_1fr]">
        {/* Left: info */}
        <div className="space-y-4">
          <Card>
            <CardHeader><CardTitle>İş Bilgileri</CardTitle></CardHeader>
            <dl className="space-y-3 text-sm">
              {[
                { icon: Cpu,      label: 'Backend',       value: backendLabel[job.gpu_backend] ?? job.gpu_backend },
                { icon: Activity, label: 'Bellek Limiti', value: job.memory_limit_mb ? formatMB(job.memory_limit_mb) : '—' },
                { icon: Clock,    label: 'Max Süre',      value: job.max_duration_secs ? `${(job.max_duration_secs/3600).toFixed(1)} saat` : '—' },
              ].map(row => (
                <div key={row.label} className="flex items-center gap-3">
                  <row.icon className="h-4 w-4 text-slate-500 shrink-0" />
                  <span className="text-slate-400 w-28 shrink-0">{row.label}</span>
                  <span className="text-slate-200">{row.value}</span>
                </div>
              ))}
              {job.worker_peer_id && (
                <div className="flex items-center gap-3">
                  <Cpu className="h-4 w-4 text-slate-500 shrink-0" />
                  <span className="text-slate-400 w-28 shrink-0">Worker</span>
                  <span className="font-mono text-xs text-slate-400">{shorten(job.worker_peer_id)}</span>
                </div>
              )}
              {job.cu_price && (
                <div className="flex items-center gap-3">
                  <span className="h-4 w-4 shrink-0" />
                  <span className="text-slate-400 w-28 shrink-0">CU Maliyeti</span>
                  <span className="font-semibold text-indigo-300">{formatCU(job.cu_price)}</span>
                </div>
              )}
            </dl>

            <div className="mt-4 pt-4 border-t border-slate-700 space-y-1.5 text-xs text-slate-500">
              <p>Oluşturulma: {formatDate(job.created_at)}</p>
              {job.assigned_at  && <p>Atanma: {formatDate(job.assigned_at)}</p>}
              {job.started_at   && <p>Başlama: {formatDate(job.started_at)}</p>}
              {job.finished_at  && <p>Bitiş: {formatDate(job.finished_at)}</p>}
            </div>

            {job.error_message && (
              <div className="mt-4 rounded-lg border border-red-700/50 bg-red-950/30 p-3 text-xs text-red-300">
                Hata: {job.error_message}
              </div>
            )}
          </Card>

          {/* Timeline */}
          <Card>
            <CardHeader><CardTitle>Durum Zaman Çizelgesi</CardTitle></CardHeader>
            <ol className="space-y-3">
              {TIMELINE_STEPS.map((step, i) => {
                const past    = i < timelineIdx
                const current = i === timelineIdx
                const failed  = ['failed', 'cancelled'].includes(job.status) && i === timelineIdx
                return (
                  <li key={step.key} className="flex items-center gap-3">
                    <div className={`flex h-6 w-6 shrink-0 items-center justify-center rounded-full border text-xs ${
                      failed  ? 'border-red-500 bg-red-950 text-red-400' :
                      past    ? 'border-emerald-500 bg-emerald-950 text-emerald-400' :
                      current ? 'border-indigo-500 bg-indigo-950 text-indigo-400' :
                                'border-slate-700 bg-slate-800 text-slate-600'
                    }`}>
                      {past ? <Check className="h-3 w-3" /> : i + 1}
                    </div>
                    <span className={`text-sm ${
                      current ? 'text-slate-100 font-medium' :
                      past    ? 'text-slate-400' :
                                'text-slate-600'
                    }`}>{step.label}</span>
                  </li>
                )
              })}
            </ol>
          </Card>
        </div>

        {/* Right: terminal */}
        <div className="h-[600px]">
          <Terminal jobId={id} jobStatus={job.status} />
        </div>
      </div>
    </div>
  )
}
