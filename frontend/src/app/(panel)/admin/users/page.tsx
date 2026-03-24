'use client'

import { useState } from 'react'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { adminApi, computeUnitsApi } from '@/lib/api'
import { useRequireAdmin } from '@/lib/auth'

export default function AdminUsersPage() {
  const user = useRequireAdmin()
  const queryClient = useQueryClient()
  const [search, setSearch] = useState('')
  const [roleFilter, setRoleFilter] = useState('')
  const [page, setPage] = useState(0)
  const [allocateModal, setAllocateModal] = useState<{ userId: string; email: string } | null>(null)

  const { data, isLoading, error } = useQuery({
    queryKey: ['admin-users', search, roleFilter, page],
    queryFn: () =>
      adminApi.users({
        search: search || undefined,
        role: roleFilter || undefined,
        limit: 50,
        offset: page * 50,
      }).then((r) => r.data),
    enabled: !!user,
  })

  if (!user) return null

  return (
    <div>
      <div className="mb-8">
        <h1 className="text-2xl font-bold text-slate-100">Kullanıcı Yönetimi</h1>
        <p className="text-slate-400 mt-1">Tüm kayıtlı kullanıcıları görüntüleyin ve yönetin</p>
      </div>

      <div className="flex gap-4 mb-6">
        <input
          type="text"
          placeholder="E-posta ile ara..."
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          className="bg-slate-800 border border-slate-700 rounded-lg px-4 py-2 text-slate-100 placeholder:text-slate-500 focus:outline-none focus:ring-2 focus:ring-indigo-500 flex-1"
        />
        <select
          value={roleFilter}
          onChange={(e) => setRoleFilter(e.target.value)}
          className="bg-slate-800 border border-slate-700 rounded-lg px-4 py-2 text-slate-100 focus:outline-none focus:ring-2 focus:ring-indigo-500"
        >
          <option value="">Tüm Roller</option>
          <option value="hirer">İş Veren</option>
          <option value="worker">İşçi</option>
          <option value="both">Her İkisi</option>
          <option value="admin">Admin</option>
        </select>
      </div>

      {error && (
        <div className="bg-red-500/10 border border-red-500/20 rounded-xl p-4 mb-6 text-red-400">
          Kullanıcılar yüklenirken hata oluştu:{' '}
          {(error as { response?: { data?: { error?: string } } })?.response?.data?.error ??
            (error as Error).message}
        </div>
      )}

      {isLoading && (
        <div className="space-y-3">
          {[...Array(5)].map((_, i) => (
            <div key={i} className="animate-pulse bg-slate-800 rounded-xl h-16" />
          ))}
        </div>
      )}

      {!isLoading && (
        <div className="bg-slate-900 border border-slate-800 rounded-xl overflow-hidden">
          <table className="w-full">
            <thead>
              <tr className="bg-slate-800/50 border-b border-slate-700">
                <th className="text-left px-6 py-3 text-xs uppercase tracking-wider text-slate-400">E-posta</th>
                <th className="text-left px-6 py-3 text-xs uppercase tracking-wider text-slate-400">Rol</th>
                <th className="text-left px-6 py-3 text-xs uppercase tracking-wider text-slate-400">CU Bakiye</th>
                <th className="text-left px-6 py-3 text-xs uppercase tracking-wider text-slate-400">İş Sayısı</th>
                <th className="text-left px-6 py-3 text-xs uppercase tracking-wider text-slate-400">Kayıt Tarihi</th>
                <th className="text-left px-6 py-3 text-xs uppercase tracking-wider text-slate-400">İşlemler</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-800">
              {(data?.users ?? []).length === 0 && (
                <tr>
                  <td colSpan={6} className="text-center py-12 text-slate-500">
                    Kullanıcı bulunamadı
                  </td>
                </tr>
              )}
              {(data?.users ?? []).map((u: {
                id: string
                email: string
                role: string
                cu_balance?: number
                job_count?: number
                created_at: string
              }) => (
                <tr key={u.id} className="hover:bg-slate-800/50 transition-colors">
                  <td className="px-6 py-4 text-slate-200">{u.email}</td>
                  <td className="px-6 py-4">
                    <RoleBadge role={u.role} />
                  </td>
                  <td className="px-6 py-4 text-slate-300 font-mono text-sm">
                    {(u.cu_balance ?? 0).toLocaleString()} CU
                  </td>
                  <td className="px-6 py-4 text-slate-400">{u.job_count ?? 0}</td>
                  <td className="px-6 py-4 text-slate-400 text-sm">
                    {new Date(u.created_at).toLocaleDateString('tr-TR')}
                  </td>
                  <td className="px-6 py-4">
                    <button
                      onClick={() => setAllocateModal({ userId: u.id, email: u.email })}
                      className="bg-indigo-600 hover:bg-indigo-500 text-white text-sm px-3 py-1.5 rounded-lg transition-colors"
                    >
                      CU Tahsis Et
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          {(data?.total ?? 0) > 50 && (
            <div className="flex items-center justify-between px-6 py-4 border-t border-slate-800">
              <span className="text-sm text-slate-400">Toplam {data?.total} kullanıcı</span>
              <div className="flex gap-2">
                <button
                  disabled={page === 0}
                  onClick={() => setPage((p) => p - 1)}
                  className="px-3 py-1.5 bg-slate-800 rounded-lg text-sm disabled:opacity-50"
                >
                  ← Önceki
                </button>
                <button
                  disabled={(page + 1) * 50 >= (data?.total ?? 0)}
                  onClick={() => setPage((p) => p + 1)}
                  className="px-3 py-1.5 bg-slate-800 rounded-lg text-sm disabled:opacity-50"
                >
                  Sonraki →
                </button>
              </div>
            </div>
          )}
        </div>
      )}

      {allocateModal && (
        <AllocateCUModal
          userId={allocateModal.userId}
          email={allocateModal.email}
          onClose={() => setAllocateModal(null)}
          onSuccess={() => {
            setAllocateModal(null)
            queryClient.invalidateQueries({ queryKey: ['admin-users'] })
          }}
        />
      )}
    </div>
  )
}

function RoleBadge({ role }: { role: string }) {
  const labels: Record<string, string> = {
    hirer: 'İş Veren',
    worker: 'İşçi',
    both: 'Her İkisi',
    admin: 'Admin',
  }
  const colors: Record<string, string> = {
    hirer: 'bg-blue-500/10 text-blue-400 border-blue-500/20',
    worker: 'bg-emerald-500/10 text-emerald-400 border-emerald-500/20',
    both: 'bg-indigo-500/10 text-indigo-400 border-indigo-500/20',
    admin: 'bg-amber-500/10 text-amber-400 border-amber-500/20',
  }
  return (
    <span className={`text-xs px-2 py-0.5 rounded-full border ${colors[role] ?? colors.hirer}`}>
      {labels[role] ?? role}
    </span>
  )
}

function AllocateCUModal({
  userId,
  email,
  onClose,
  onSuccess,
}: {
  userId: string
  email: string
  onClose: () => void
  onSuccess: () => void
}) {
  const [amount, setAmount] = useState('')
  const [reason, setReason] = useState('')
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState('')

  const handleSubmit = async () => {
    const n = parseInt(amount)
    if (!n || n <= 0) {
      setError('Geçerli bir miktar girin')
      return
    }
    if (!reason.trim()) {
      setError('Gerekçe zorunludur')
      return
    }
    setLoading(true)
    setError('')
    try {
      await computeUnitsApi.allocate({ user_id: userId, amount: n, description: reason.trim() })
      onSuccess()
    } catch (e: unknown) {
      const err = e as { response?: { data?: { error?: string } } }
      setError(err.response?.data?.error ?? 'Tahsis başarısız oldu')
    } finally {
      setLoading(false)
    }
  }

  return (
    <div className="fixed inset-0 bg-black/60 backdrop-blur-sm flex items-center justify-center z-50 p-4">
      <div className="bg-slate-900 border border-slate-700 rounded-2xl p-6 w-full max-w-md shadow-2xl">
        <h2 className="text-lg font-bold text-slate-100 mb-1">CU Tahsis Et</h2>
        <p className="text-sm text-slate-400 mb-6">{email}</p>
        <div className="space-y-4">
          <div>
            <label className="text-sm font-medium text-slate-300 block mb-1.5">Miktar (CU)</label>
            <input
              type="number"
              min="1"
              max="1000000"
              value={amount}
              onChange={(e) => setAmount(e.target.value)}
              placeholder="örn. 100"
              className="w-full bg-slate-800 border border-slate-700 rounded-lg px-4 py-2.5 text-slate-100 placeholder:text-slate-500 focus:outline-none focus:ring-2 focus:ring-indigo-500"
            />
          </div>
          <div>
            <label className="text-sm font-medium text-slate-300 block mb-1.5">Gerekçe</label>
            <input
              type="text"
              value={reason}
              onChange={(e) => setReason(e.target.value)}
              placeholder="örn. Araştırma projesi için tahsis"
              className="w-full bg-slate-800 border border-slate-700 rounded-lg px-4 py-2.5 text-slate-100 placeholder:text-slate-500 focus:outline-none focus:ring-2 focus:ring-indigo-500"
            />
          </div>
          {error && (
            <p className="text-sm text-red-400 bg-red-500/10 border border-red-500/20 rounded-lg p-3">
              {error}
            </p>
          )}
        </div>
        <div className="flex gap-3 mt-6">
          <button
            onClick={onClose}
            className="flex-1 px-4 py-2.5 bg-slate-800 hover:bg-slate-700 text-slate-300 rounded-lg transition-colors text-sm"
          >
            İptal
          </button>
          <button
            onClick={handleSubmit}
            disabled={loading}
            className="flex-1 px-4 py-2.5 bg-indigo-600 hover:bg-indigo-500 text-white rounded-lg transition-colors text-sm font-medium disabled:opacity-50 disabled:cursor-not-allowed"
          >
            {loading ? 'Tahsis Ediliyor...' : 'Tahsis Et'}
          </button>
        </div>
      </div>
    </div>
  )
}
