'use client'

import { useQuery } from '@tanstack/react-query'
import Link from 'next/link'
import { Coins, Play, CheckCircle, Cpu, ArrowRight } from 'lucide-react'
import { computeUnitsApi, jobsApi, workersApi } from '@/lib/api'
import { useAuth } from '@/lib/auth'
import { Card } from '@/components/ui/Card'
import { JobStatusBadge } from '@/components/jobs/JobStatusBadge'
import { PageSpinner } from '@/components/ui/Spinner'
import { formatCU, formatDate, shorten, backendLabel } from '@/lib/utils'
import type { JobStatus } from '@/types'

function StatCard({ icon: Icon, label, value, color }: {
  icon: React.ComponentType<{ className?: string }>
  label: string
  value: string | number
  color: string
}) {
  return (
    <Card className="flex items-center gap-4">
      <div className={`flex h-12 w-12 shrink-0 items-center justify-center rounded-xl ${color}`}>
        <Icon className="h-6 w-6" />
      </div>
      <div>
        <p className="text-sm text-slate-400">{label}</p>
        <p className="text-2xl font-bold text-slate-100">{value}</p>
      </div>
    </Card>
  )
}

export default function PanelPage() {
  const { user } = useAuth()

  const { data: balance } = useQuery({
    queryKey: ['balance'],
    queryFn: () => computeUnitsApi.balance().then(r => r.data),
    enabled: !!user,
    refetchInterval: 30_000,
  })

  const { data: jobs, isLoading: jobsLoading } = useQuery({
    queryKey: ['jobs'],
    queryFn: () => jobsApi.list({ limit: 20 }).then(r => r.data),
    enabled: !!user,
    refetchInterval: 10_000,
  })

  const { data: workers } = useQuery({
    queryKey: ['workers', { online_only: true }],
    queryFn: () => workersApi.list({ online_only: true, limit: 3 }).then(r => r.data),
    enabled: !!user,
    refetchInterval: 30_000,
  })

  const jobList  = Array.isArray(jobs) ? jobs : []
  const running  = jobList.filter(j => j.status === 'running').length
  const done     = jobList.filter(j => j.status === 'completed').length
  const recent5  = jobList.slice(0, 5)

  if (jobsLoading) return <PageSpinner />

  return (
    <div className="space-y-8">
      <div>
        <h1 className="text-2xl font-bold text-slate-100">Genel Bakış</h1>
        <p className="text-slate-400 mt-1">Merhaba, {user?.email}</p>
      </div>

      {/* Stats */}
      <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-4">
        <StatCard icon={Coins}       label="CU Bakiyesi"       value={formatCU(balance?.cu_available)} color="bg-indigo-500/10 text-indigo-400" />
        <StatCard icon={Play}        label="Aktif İşler"        value={running}                         color="bg-blue-500/10 text-blue-400" />
        <StatCard icon={CheckCircle} label="Tamamlanan İşler"   value={done}                            color="bg-emerald-500/10 text-emerald-400" />
        <StatCard icon={Cpu}         label="Çevrimiçi Worker"   value={workers?.length ?? 0}            color="bg-amber-500/10 text-amber-400" />
      </div>

      <div className="grid gap-6 lg:grid-cols-2">
        {/* Recent jobs */}
        <Card>
          <div className="flex items-center justify-between mb-4">
            <h2 className="font-semibold text-slate-100">Son İşler</h2>
            <Link href="/jobs" className="flex items-center gap-1 text-sm text-indigo-400 hover:text-indigo-300">
              Tümünü Gör <ArrowRight className="h-4 w-4" />
            </Link>
          </div>
          {recent5.length === 0 ? (
            <p className="text-center text-sm text-slate-500 py-8">
              Henüz bir iş göndermediniz.{' '}
              <Link href="/jobs/new" className="text-indigo-400 hover:underline">İş oluştur →</Link>
            </p>
          ) : (
            <div className="divide-y divide-slate-700">
              {recent5.map(job => (
                <Link key={job.id} href={`/jobs/${job.id}`} className="flex items-center gap-3 py-3 hover:text-slate-200 transition-colors">
                  <span className="flex-1 font-mono text-xs text-slate-400">{shorten(job.id, 8)}</span>
                  <JobStatusBadge status={job.status as JobStatus} />
                  <span className="text-xs text-slate-500">{backendLabel[job.gpu_backend]}</span>
                  {job.cu_price && <span className="text-xs text-indigo-300">{formatCU(job.cu_price)}</span>}
                </Link>
              ))}
            </div>
          )}
        </Card>

        {/* Top workers */}
        <Card>
          <div className="flex items-center justify-between mb-4">
            <h2 className="font-semibold text-slate-100">Aktif Worker&apos;lar</h2>
            <Link href="/rent" className="flex items-center gap-1 text-sm text-indigo-400 hover:text-indigo-300">
              Tümünü Gör <ArrowRight className="h-4 w-4" />
            </Link>
          </div>
          {!workers || workers.length === 0 ? (
            <p className="text-center text-sm text-slate-500 py-8">Şu an çevrimiçi worker bulunmuyor.</p>
          ) : (
            <div className="divide-y divide-slate-700">
              {workers.map(w => (
                <div key={w.peer_id} className="flex items-center gap-3 py-3">
                  <div className="flex-1 min-w-0">
                    <p className="font-mono text-xs text-slate-300 truncate">{shorten(w.peer_id)}</p>
                    <p className="text-xs text-slate-500">
                      {w.capabilities?.gpus?.[0]?.name ?? 'CPU'}
                    </p>
                  </div>
                  <div className="text-right">
                    <p className="text-xs font-medium text-emerald-400">{w.uptime_score.toFixed(1)}% çevrimiçi</p>
                    <p className="text-xs text-slate-500">{w.jobs_completed} iş</p>
                  </div>
                </div>
              ))}
            </div>
          )}
        </Card>
      </div>

      {/* Quick links */}
      <Card className="bg-gradient-to-r from-indigo-950/40 to-slate-800">
        <div className="flex flex-col sm:flex-row items-start sm:items-center justify-between gap-4">
          <div>
            <h3 className="font-semibold text-slate-100">İlk işinizi gönderin</h3>
            <p className="text-sm text-slate-400 mt-1">
              Python kodunuzu yükleyin, GPU seçin ve saniyeler içinde başlatın.
            </p>
          </div>
          <Link
            href="/jobs/new"
            className="shrink-0 inline-flex items-center gap-2 rounded-lg bg-indigo-600 hover:bg-indigo-500 px-5 py-2.5 text-sm font-semibold transition-colors"
          >
            Yeni İş Oluştur <ArrowRight className="h-4 w-4" />
          </Link>
        </div>
      </Card>
    </div>
  )
}
