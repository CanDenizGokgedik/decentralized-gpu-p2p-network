'use client'

import { useQuery } from '@tanstack/react-query'
import { adminApi } from '@/lib/api'
import { useRequireAdmin } from '@/lib/auth'
import { Users, Cpu, Briefcase, CheckCircle, TrendingUp, Zap, Activity } from 'lucide-react'

function BarChart({ data, height = 120 }: { data: Array<{ label: string; value: number; color?: string }>; height?: number }) {
  const max = Math.max(...data.map((d) => d.value), 1)
  return (
    <div className="flex items-end gap-1 w-full" style={{ height }}>
      {data.map((d, i) => (
        <div key={i} className="flex-1 flex flex-col items-center gap-1 group relative">
          <div className="absolute bottom-full mb-1 opacity-0 group-hover:opacity-100 transition-opacity bg-slate-800 text-xs text-slate-200 px-2 py-1 rounded whitespace-nowrap pointer-events-none z-10">
            {d.label}: {d.value}
          </div>
          <div
            className={`w-full rounded-t transition-all duration-500 ${d.value === 0 ? 'bg-slate-800' : d.color ?? 'bg-indigo-500'}`}
            style={{ height: `${(d.value / max) * (height - 24)}px`, minHeight: d.value > 0 ? '4px' : '2px' }}
          />
        </div>
      ))}
    </div>
  )
}

function DonutChart({ data }: { data: Array<{ label: string; value: number; color: string }> }) {
  const total = data.reduce((s, d) => s + d.value, 0) || 1
  let cumulative = 0
  const radius = 40
  const cx = 60,
    cy = 60
  const circumference = 2 * Math.PI * radius
  return (
    <div className="flex items-center gap-6">
      <svg width={120} height={120} viewBox="0 0 120 120">
        <circle cx={cx} cy={cy} r={radius} fill="none" stroke="#1e293b" strokeWidth={18} />
        {data.map((d, i) => {
          const pct = d.value / total
          const offset = circumference * (1 - cumulative)
          const dash = circumference * pct
          cumulative += pct
          return (
            <circle
              key={i}
              cx={cx}
              cy={cy}
              r={radius}
              fill="none"
              stroke={d.color}
              strokeWidth={18}
              strokeDasharray={`${dash} ${circumference - dash}`}
              strokeDashoffset={offset}
              transform={`rotate(-90 ${cx} ${cy})`}
            />
          )
        })}
        <text x={cx} y={cy - 6} textAnchor="middle" fill="#e2e8f0" fontSize={18} fontWeight="bold">
          {total}
        </text>
        <text x={cx} y={cy + 12} textAnchor="middle" fill="#64748b" fontSize={9}>
          toplam
        </text>
      </svg>
      <div className="space-y-2">
        {data.map((d, i) => (
          <div key={i} className="flex items-center gap-2">
            <div className="w-2.5 h-2.5 rounded-full shrink-0" style={{ backgroundColor: d.color }} />
            <span className="text-xs text-slate-400">{d.label}</span>
            <span className="text-xs font-semibold text-slate-200 ml-1">{d.value}</span>
            <span className="text-xs text-slate-600">({((d.value / total) * 100).toFixed(0)}%)</span>
          </div>
        ))}
      </div>
    </div>
  )
}

function StatCard({ icon: Icon, label, value, sub, color = 'indigo' }: {
  icon: React.ElementType
  label: string
  value: string | number
  sub?: string
  color?: 'indigo' | 'emerald' | 'amber' | 'blue' | 'purple'
}) {
  const colors: Record<string, string> = {
    indigo: 'bg-indigo-500/10 text-indigo-400',
    emerald: 'bg-emerald-500/10 text-emerald-400',
    amber: 'bg-amber-500/10 text-amber-400',
    blue: 'bg-blue-500/10 text-blue-400',
    purple: 'bg-purple-500/10 text-purple-400',
  }
  return (
    <div className="bg-slate-900 border border-slate-800 rounded-xl p-5 hover:border-slate-700 transition-colors">
      <div className={`w-10 h-10 rounded-lg flex items-center justify-center mb-4 ${colors[color]}`}>
        <Icon className="w-5 h-5" />
      </div>
      <p className="text-2xl font-bold text-slate-100">{value}</p>
      <p className="text-sm text-slate-400 mt-0.5">{label}</p>
      {sub && <p className="text-xs text-slate-600 mt-1">{sub}</p>}
    </div>
  )
}

export default function AdminDashboardPage() {
  const admin = useRequireAdmin()
  const { data: stats, isLoading } = useQuery({
    queryKey: ['admin-stats'],
    queryFn: () => adminApi.stats().then((r) => r.data),
    refetchInterval: 30_000,
    enabled: !!admin,
  })

  if (!admin) return null

  const last14Days = Array.from({ length: 14 }, (_, i) => {
    const d = new Date()
    d.setDate(d.getDate() - (13 - i))
    const dayStr = d.toISOString().split('T')[0]
    const found = (stats?.jobs_by_day ?? []).find((r: { day: string }) => r.day === dayStr)
    return {
      label: d.toLocaleDateString('tr-TR', { month: 'short', day: 'numeric' }),
      value: found?.count ?? 0,
    }
  })

  const backendColors: Record<string, string> = {
    cuda: '#6366f1',
    rocm: '#f59e0b',
    metal: '#8b5cf6',
    cpu_only: '#64748b',
  }
  const backendLabels: Record<string, string> = {
    cuda: 'NVIDIA CUDA',
    rocm: 'AMD ROCm',
    metal: 'Apple Metal',
    cpu_only: 'CPU',
  }

  const donutData = (stats?.jobs_by_backend ?? []).map((b: { backend: string; count: number }) => ({
    label: backendLabels[b.backend] ?? b.backend,
    value: b.count,
    color: backendColors[b.backend] ?? '#64748b',
  }))

  return (
    <div>
      <div className="mb-8 flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-slate-100">Yönetim Paneli</h1>
          <p className="text-slate-400 mt-1">Sistem genel görünümü</p>
        </div>
        <div className="flex items-center gap-2 text-xs text-slate-500">
          <Activity className="w-3.5 h-3.5 animate-pulse text-emerald-400" />
          30s güncelleme
        </div>
      </div>

      {isLoading ? (
        <div className="grid grid-cols-2 lg:grid-cols-4 gap-4 mb-8">
          {[...Array(8)].map((_, i) => (
            <div key={i} className="animate-pulse bg-slate-800 rounded-xl h-28" />
          ))}
        </div>
      ) : (
        <div className="grid grid-cols-2 lg:grid-cols-4 gap-4 mb-8">
          <StatCard icon={Users} label="Toplam Kullanıcı" value={stats?.users_total ?? 0} color="blue" />
          <StatCard icon={Cpu} label="Aktif Worker" value={stats?.workers_online ?? 0} sub="Şu an bağlı" color="emerald" />
          <StatCard icon={Activity} label="Çalışan İş" value={stats?.jobs_running ?? 0} sub="Aktif eğitim" color="indigo" />
          <StatCard icon={CheckCircle} label="Bugün Tamamlanan" value={stats?.jobs_today ?? 0} color="emerald" />
          <StatCard icon={Briefcase} label="Toplam İş" value={stats?.jobs_total ?? 0} color="purple" />
          <StatCard icon={Zap} label="Tahsis Edilen CU" value={(stats?.cu_allocated_total ?? 0).toLocaleString('tr-TR')} sub="Tüm zamanlarda" color="amber" />
          <StatCard icon={TrendingUp} label="Tüketilen CU" value={(stats?.cu_consumed_total ?? 0).toLocaleString('tr-TR')} color="indigo" />
          <StatCard
            icon={Activity}
            label="Kullanım Oranı"
            value={stats?.cu_allocated_total ? `${(((stats.cu_consumed_total ?? 0) / stats.cu_allocated_total) * 100).toFixed(1)}%` : '0%'}
            sub="CU verimliliği"
            color="blue"
          />
        </div>
      )}

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6 mb-8">
        <div className="lg:col-span-2 bg-slate-900 border border-slate-800 rounded-xl p-6">
          <h2 className="font-semibold text-slate-200 mb-1">Son 14 Gün — İş Aktivitesi</h2>
          <p className="text-xs text-slate-500 mb-6">Günlük gönderilen iş sayısı</p>
          {isLoading ? (
            <div className="animate-pulse bg-slate-800 rounded h-32" />
          ) : (
            <>
              <BarChart height={140} data={last14Days.map((d) => ({ ...d, color: 'bg-indigo-500' }))} />
              <div className="flex gap-1 mt-1">
                {last14Days.map((d, i) => (
                  <div key={i} className="flex-1 text-center text-[9px] text-slate-600 truncate">
                    {i % 2 === 0 ? d.label.split(' ')[0] : ''}
                  </div>
                ))}
              </div>
            </>
          )}
        </div>
        <div className="bg-slate-900 border border-slate-800 rounded-xl p-6">
          <h2 className="font-semibold text-slate-200 mb-1">Backend Dağılımı</h2>
          <p className="text-xs text-slate-500 mb-6">GPU türüne göre işler</p>
          {isLoading ? (
            <div className="animate-pulse bg-slate-800 rounded-full h-32 w-32 mx-auto" />
          ) : donutData.length > 0 ? (
            <DonutChart data={donutData} />
          ) : (
            <div className="flex flex-col items-center justify-center h-32 text-slate-600">
              <Briefcase className="w-8 h-8 mb-2" />
              <p className="text-sm">Henüz iş yok</p>
            </div>
          )}
        </div>
      </div>

      <div className="grid grid-cols-1 sm:grid-cols-2 gap-4 mb-8">
        <a href="/admin/users" className="bg-slate-900 border border-slate-800 hover:border-indigo-500/50 rounded-xl p-5 transition-all group flex items-center justify-between">
          <div>
            <p className="font-semibold text-slate-200 group-hover:text-indigo-300 transition-colors">Kullanıcı Yönetimi</p>
            <p className="text-sm text-slate-500 mt-0.5">{stats?.users_total ?? 0} kayıtlı kullanıcı</p>
          </div>
          <Users className="w-5 h-5 text-slate-600 group-hover:text-indigo-400 transition-colors" />
        </a>
        <a href="/admin/users" className="bg-slate-900 border border-slate-800 hover:border-emerald-500/50 rounded-xl p-5 transition-all group flex items-center justify-between">
          <div>
            <p className="font-semibold text-slate-200 group-hover:text-emerald-300 transition-colors">CU Tahsis Et</p>
            <p className="text-sm text-slate-500 mt-0.5">Kullanıcılara compute unit ver</p>
          </div>
          <Zap className="w-5 h-5 text-slate-600 group-hover:text-emerald-400 transition-colors" />
        </a>
      </div>

      <BinaryManagement />
    </div>
  )
}

function BinaryManagement() {
  const [uploading, setUploading] = React.useState<Record<string, boolean>>({})
  const platforms = [
    { id: 'linux-x86_64', label: 'Linux (x86_64)', icon: Activity },
    { id: 'macos-aarch64', label: 'macOS (Apple Silicon)', icon: Cpu },
    { id: 'macos-x86_64', label: 'macOS (Intel)', icon: Cpu },
    { id: 'windows-x86_64', label: 'Windows (x86_64)', icon: Briefcase },
  ]

  const handleUpload = async (platform: string, e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0]
    if (!file) return

    setUploading(prev => ({ ...prev, [platform]: true }))
    try {
      await adminApi.uploadWorkerBinary(platform, file)
      alert(`${platform} için binary başarıyla yüklendi!`)
    } catch (err: any) {
      alert(`Yükleme hatası: ${err.response?.data?.error || err.message}`)
    } finally {
      setUploading(prev => ({ ...prev, [platform]: false }))
      e.target.value = ''
    }
  }

  return (
    <div className="bg-slate-900 border border-slate-800 rounded-xl p-6">
      <h2 className="font-semibold text-slate-200 mb-1">İşçi Yazılım Yönetimi</h2>
      <p className="text-xs text-slate-500 mb-6">Farklı platformlar için derlenmiş worker binary dosyalarını sunucuya yükle</p>

      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
        {platforms.map((p) => (
          <div key={p.id} className="p-4 bg-slate-950 border border-slate-800 rounded-lg">
            <div className="flex items-center gap-3 mb-4">
              <div className="w-8 h-8 rounded-md bg-slate-800 flex items-center justify-center">
                <p.icon className="w-4 h-4 text-slate-400" />
              </div>
              <span className="text-xs font-medium text-slate-300">{p.label}</span>
            </div>
            
            <label className={`
              flex items-center justify-center gap-2 px-3 py-2 rounded-md text-xs font-medium transition-all cursor-pointer
              ${uploading[p.id] 
                ? 'bg-slate-800 text-slate-500 cursor-not-allowed' 
                : 'bg-indigo-600 hover:bg-indigo-500 text-white shadow-lg shadow-indigo-500/10'}
            `}>
              {uploading[p.id] ? (
                <>
                  <Activity className="w-3 h-3 animate-spin" />
                  Yükleniyor...
                </>
              ) : (
                <>
                  <Zap className="w-3 h-3" />
                  Dosya Yükle
                </>
              )}
              <input 
                type="file" 
                className="hidden" 
                onChange={(e) => handleUpload(p.id, e)} 
                disabled={uploading[p.id]}
              />
            </label>
          </div>
        ))}
      </div>
    </div>
  )
}

import React from 'react'
