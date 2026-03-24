'use client'

import { useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { useAuth } from '@/lib/auth'
import { workersApi, computeUnitsApi, jobsApi } from '@/lib/api'
import { User, Cpu, Briefcase, Zap, Shield } from 'lucide-react'
import type { Job, Transaction } from '@/types'

export default function AccountPage() {
  const { user, logout } = useAuth()
  const [activeTab, setActiveTab] = useState<'genel' | 'isler' | 'islemler'>('genel')

  const { data: balance } = useQuery({
    queryKey: ['balance'],
    queryFn: () => computeUnitsApi.balance().then((r) => r.data),
    enabled: !!user,
  })

  const { data: myJobs } = useQuery({
    queryKey: ['my-jobs-account'],
    queryFn: () => jobsApi.list({ limit: 10, offset: 0 }).then((r) => r.data),
    enabled: activeTab === 'isler' && !!user,
  })

  const { data: txHistory } = useQuery({
    queryKey: ['tx-history'],
    queryFn: () => computeUnitsApi.transactions({ limit: 20 }).then((r) => r.data),
    enabled: activeTab === 'islemler' && !!user,
  })

  const { data: workerInfo } = useQuery({
    queryKey: ['my-worker-account'],
    queryFn: () => workersApi.me().then((r) => r.data).catch(() => null),
    enabled: !!user,
    retry: false,
  })

  if (!user) return null

  const roleLabels: Record<string, string> = {
    hirer: 'İş Veren',
    worker: 'İşçi',
    both: 'İş Veren & İşçi',
    admin: 'Yönetici',
  }

  const statusColors: Record<string, string> = {
    pending: 'text-amber-400',
    assigned: 'text-blue-400',
    running: 'text-indigo-400',
    completed: 'text-emerald-400',
    failed: 'text-red-400',
    cancelled: 'text-slate-500',
  }

  const statusLabels: Record<string, string> = {
    pending: 'Bekliyor',
    assigned: 'Atandı',
    running: 'Çalışıyor',
    completed: 'Tamamlandı',
    failed: 'Başarısız',
    cancelled: 'İptal',
  }

  const txLabels: Record<string, string> = {
    allocation: 'Yönetici Tahsisi',
    job_debit: 'İş Kesintisi',
    job_credit: 'İş Kazancı',
    job_refund: 'İş İadesi',
  }

  return (
    <div className="max-w-3xl">
      {/* Profile header */}
      <div className="bg-slate-900 border border-slate-800 rounded-2xl p-6 mb-6">
        <div className="flex items-start gap-5">
          <div className="w-16 h-16 bg-indigo-500/20 rounded-2xl flex items-center justify-center shrink-0">
            <span className="text-2xl font-bold text-indigo-400">{user.email[0].toUpperCase()}</span>
          </div>
          <div className="flex-1 min-w-0">
            <div className="flex items-center gap-3 flex-wrap">
              <h1 className="text-xl font-bold text-slate-100 truncate">{user.email}</h1>
              <span
                className={`text-xs px-2.5 py-1 rounded-full border font-medium ${
                  user.role === 'admin'
                    ? 'bg-amber-500/10 text-amber-400 border-amber-500/20'
                    : 'bg-indigo-500/10 text-indigo-400 border-indigo-500/20'
                }`}
              >
                {roleLabels[user.role] ?? user.role}
              </span>
            </div>
          </div>
        </div>
        <div className="grid grid-cols-3 gap-4 mt-6 pt-6 border-t border-slate-800">
          <div className="text-center">
            <p className="text-xl font-bold text-slate-100">{(balance?.cu_available ?? 0).toLocaleString('tr-TR')}</p>
            <p className="text-xs text-slate-500 mt-0.5">Kullanılabilir CU</p>
          </div>
          <div className="text-center border-x border-slate-800">
            <p className="text-xl font-bold text-slate-100">{myJobs?.total ?? 0}</p>
            <p className="text-xs text-slate-500 mt-0.5">Toplam İş</p>
          </div>
          <div className="text-center">
            <p className={`text-xl font-bold ${workerInfo?.is_online ? 'text-emerald-400' : 'text-slate-500'}`}>
              {workerInfo?.is_online ? '● Aktif' : '○ Pasif'}
            </p>
            <p className="text-xs text-slate-500 mt-0.5">Worker</p>
          </div>
        </div>
      </div>

      {/* Tabs */}
      <div className="flex gap-1 bg-slate-900 border border-slate-800 rounded-xl p-1 mb-6">
        {(
          [
            { id: 'genel', label: 'Genel Bilgiler', icon: User },
            { id: 'isler', label: 'İşlerim', icon: Briefcase },
            { id: 'islemler', label: 'CU Geçmişi', icon: Zap },
          ] as const
        ).map((tab) => (
          <button
            key={tab.id}
            onClick={() => setActiveTab(tab.id)}
            className={`flex-1 flex items-center justify-center gap-2 py-2.5 rounded-lg text-sm font-medium transition-colors ${
              activeTab === tab.id ? 'bg-slate-800 text-slate-100' : 'text-slate-500 hover:text-slate-300'
            }`}
          >
            <tab.icon className="w-4 h-4" />
            {tab.label}
          </button>
        ))}
      </div>

      {activeTab === 'genel' && (
        <div className="space-y-4">
          <div className="bg-slate-900 border border-slate-800 rounded-xl p-5">
            <h2 className="font-semibold text-slate-200 mb-4 flex items-center gap-2">
              <Zap className="w-4 h-4 text-amber-400" />
              Compute Unit Bakiyesi
            </h2>
            <div className="grid grid-cols-3 gap-4">
              {[
                { label: 'Kullanılabilir', value: balance?.cu_available ?? 0, color: 'text-emerald-400' },
                { label: 'Rezerve', value: balance?.cu_reserved ?? 0, color: 'text-amber-400' },
                { label: 'Toplam', value: balance?.cu_balance ?? 0, color: 'text-slate-200' },
              ].map((b) => (
                <div key={b.label} className="bg-slate-800/50 rounded-lg p-3 text-center">
                  <p className={`text-lg font-bold ${b.color}`}>{b.value.toLocaleString('tr-TR')}</p>
                  <p className="text-xs text-slate-500 mt-0.5">{b.label} CU</p>
                </div>
              ))}
            </div>
          </div>

          {(user.role === 'worker' || user.role === 'both') && (
            <div className="bg-slate-900 border border-slate-800 rounded-xl p-5">
              <h2 className="font-semibold text-slate-200 mb-4 flex items-center gap-2">
                <Cpu className="w-4 h-4 text-indigo-400" />
                Worker Bilgileri
              </h2>
              {workerInfo ? (
                <div className="space-y-3">
                  <div className="flex items-center justify-between">
                    <span className="text-sm text-slate-400">Bağlantı</span>
                    <span className={`text-sm font-medium flex items-center gap-1.5 ${workerInfo.is_online ? 'text-emerald-400' : 'text-slate-500'}`}>
                      <span className={`w-2 h-2 rounded-full ${workerInfo.is_online ? 'bg-emerald-400 animate-pulse' : 'bg-slate-600'}`} />
                      {workerInfo.is_online ? 'Çevrimiçi' : 'Çevrimdışı'}
                    </span>
                  </div>
                  <div className="flex items-center justify-between">
                    <span className="text-sm text-slate-400">Peer ID</span>
                    <span className="text-xs font-mono text-slate-500 break-all text-right max-w-[200px]">{workerInfo.peer_id}</span>
                  </div>
                </div>
              ) : (
                <div className="text-center py-6">
                  <Cpu className="w-8 h-8 text-slate-700 mx-auto mb-2" />
                  <p className="text-sm text-slate-500">Henüz bağlanmadınız</p>
                  <a href="/account/client" className="text-xs text-indigo-400 hover:text-indigo-300 mt-2 inline-block">
                    Worker olmak için tıklayın →
                  </a>
                </div>
              )}
            </div>
          )}

          <div className="bg-slate-900 border border-slate-800 rounded-xl p-5">
            <h2 className="font-semibold text-slate-200 mb-4 flex items-center gap-2">
              <Shield className="w-4 h-4 text-red-400" />
              Hesap
            </h2>
            <button
              onClick={logout}
              className="px-4 py-2 bg-red-500/10 hover:bg-red-500/20 border border-red-500/20 text-red-400 rounded-lg text-sm transition-colors"
            >
              Çıkış Yap
            </button>
          </div>
        </div>
      )}

      {activeTab === 'isler' && (
        <div className="bg-slate-900 border border-slate-800 rounded-xl overflow-hidden">
          {(myJobs?.jobs ?? []).length === 0 ? (
            <div className="text-center py-12">
              <Briefcase className="w-10 h-10 text-slate-700 mx-auto mb-3" />
              <p className="text-slate-400">Henüz iş yok</p>
              <a href="/jobs/new" className="text-sm text-indigo-400 hover:text-indigo-300 mt-2 inline-block">
                İlk işini oluştur →
              </a>
            </div>
          ) : (
            <table className="w-full">
              <thead>
                <tr className="bg-slate-800/50 border-b border-slate-700">
                  <th className="text-left px-5 py-3 text-xs uppercase tracking-wider text-slate-400">İş ID</th>
                  <th className="text-left px-5 py-3 text-xs uppercase tracking-wider text-slate-400">Durum</th>
                  <th className="text-left px-5 py-3 text-xs uppercase tracking-wider text-slate-400">Backend</th>
                  <th className="text-left px-5 py-3 text-xs uppercase tracking-wider text-slate-400">Tarih</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-slate-800">
                {(myJobs?.jobs ?? []).map((job: Job) => (
                  <tr key={job.id} onClick={() => (window.location.href = `/jobs/${job.id}`)} className="hover:bg-slate-800/50 cursor-pointer transition-colors">
                    <td className="px-5 py-3 font-mono text-xs text-slate-400">{job.id.slice(0, 8)}...</td>
                    <td className="px-5 py-3">
                      <span className={`text-sm font-medium ${statusColors[job.status] ?? 'text-slate-400'}`}>{statusLabels[job.status] ?? job.status}</span>
                    </td>
                    <td className="px-5 py-3 text-sm text-slate-400 uppercase">{job.gpu_backend ?? 'cpu'}</td>
                    <td className="px-5 py-3 text-xs text-slate-500">{new Date(job.created_at).toLocaleDateString('tr-TR')}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>
      )}

      {activeTab === 'islemler' && (
        <div className="bg-slate-900 border border-slate-800 rounded-xl overflow-hidden">
          {(txHistory?.transactions ?? []).length === 0 ? (
            <div className="text-center py-12">
              <Zap className="w-10 h-10 text-slate-700 mx-auto mb-3" />
              <p className="text-slate-400">Henüz işlem yok</p>
            </div>
          ) : (
            <table className="w-full">
              <thead>
                <tr className="bg-slate-800/50 border-b border-slate-700">
                  <th className="text-left px-5 py-3 text-xs uppercase tracking-wider text-slate-400">Tarih</th>
                  <th className="text-left px-5 py-3 text-xs uppercase tracking-wider text-slate-400">Tür</th>
                  <th className="text-right px-5 py-3 text-xs uppercase tracking-wider text-slate-400">Miktar</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-slate-800">
                {(txHistory?.transactions ?? []).map((tx: Transaction) => (
                  <tr key={tx.id} className="hover:bg-slate-800/30 transition-colors">
                    <td className="px-5 py-3 text-xs text-slate-500">
                      {new Date(tx.created_at).toLocaleDateString('tr-TR', { day: '2-digit', month: 'short', hour: '2-digit', minute: '2-digit' })}
                    </td>
                    <td className="px-5 py-3 text-sm text-slate-300">{txLabels[tx.tx_type] ?? tx.tx_type}</td>
                    <td className={`px-5 py-3 text-sm font-mono font-semibold text-right ${tx.cu_amount > 0 ? 'text-emerald-400' : 'text-red-400'}`}>
                      {tx.cu_amount > 0 ? '+' : ''}{tx.cu_amount.toLocaleString('tr-TR')} CU
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>
      )}
    </div>
  )
}
