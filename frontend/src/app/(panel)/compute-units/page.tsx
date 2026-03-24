'use client'

import { useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { useAuth } from '@/lib/auth'
import { computeUnitsApi } from '@/lib/api'
import { Card, CardHeader, CardTitle } from '@/components/ui/Card'
import { PageSpinner } from '@/components/ui/Spinner'
import { formatCU, formatDate, txTypeLabel } from '@/lib/utils'
import { Coins, Info, X, ChevronLeft, ChevronRight } from 'lucide-react'
import { BACKEND_OPTIONS } from '@/lib/constants'
import { cn } from '@/lib/utils'

const TX_TYPE_FILTERS = [
  { value: '', label: 'Tümü' },
  { value: 'purchase', label: 'Satın Alma' },
  { value: 'usage',    label: 'Kullanım' },
  { value: 'admin',    label: 'Admin' },
  { value: 'refund',   label: 'İade' },
]

const PAGE_SIZE = 20

export default function ComputeUnitsPage() {
  const { user }               = useAuth()
  const [dismissed, setDismissed] = useState(false)
  const [txType,    setTxType]    = useState('')
  const [page,      setPage]      = useState(0)

  const { data: balance, isLoading } = useQuery({
    queryKey: ['balance'],
    queryFn:  () => computeUnitsApi.balance().then(r => r.data),
    enabled:  !!user,
    refetchInterval: 30_000,
  })

  const { data: pricing } = useQuery({
    queryKey: ['pricing'],
    queryFn:  () => computeUnitsApi.pricing().then(r => r.data),
  })

  const { data: txData } = useQuery({
    queryKey: ['transactions', txType, page],
    queryFn:  () => computeUnitsApi.transactions({
      tx_type: txType || undefined,
      limit:   PAGE_SIZE,
      offset:  page * PAGE_SIZE,
    }).then(r => r.data),
    enabled: !!user,
  })

  const transactions = txData?.transactions ?? []

  if (isLoading) return <PageSpinner />

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-bold text-slate-100">Compute Units</h1>
        <p className="text-slate-400 mt-1">Bakiyenizi ve işlem geçmişinizi görüntüleyin</p>
      </div>

      {/* Info box */}
      {!dismissed && (
        <div className="flex items-start gap-3 rounded-xl border border-indigo-700/40 bg-indigo-950/30 p-4 pr-3">
          <Info className="h-5 w-5 shrink-0 text-indigo-400 mt-0.5" />
          <div className="flex-1 text-sm text-indigo-200">
            <p className="font-semibold mb-1">Compute Unit (CU) Nedir?</p>
            <p className="text-indigo-300/80">
              CU, DecentGPU platformunda GPU hesaplama gücünü temsil eden sanal para birimidir.
              İşleriniz çalıştıkça GPU türüne ve süreye göre CU harcanır.
              Yöneticinizden CU talep edebilirsiniz.
            </p>
          </div>
          <button onClick={() => setDismissed(true)} className="text-indigo-400 hover:text-indigo-200 p-1 rounded">
            <X className="h-4 w-4" />
          </button>
        </div>
      )}

      {/* Balance card */}
      <div className="grid gap-4 sm:grid-cols-3">
        <Card className="sm:col-span-1 flex items-center gap-4">
          <div className="flex h-14 w-14 shrink-0 items-center justify-center rounded-xl bg-indigo-500/10">
            <Coins className="h-7 w-7 text-indigo-400" />
          </div>
          <div>
            <p className="text-sm text-slate-400">Mevcut Bakiye</p>
            <p className="text-3xl font-bold text-indigo-300">{formatCU(balance?.cu_available)}</p>
          </div>
        </Card>
        <Card className="sm:col-span-2">
          <p className="text-sm font-medium text-slate-300 mb-3">Son İşlemler Özeti</p>
          <div className="grid grid-cols-2 gap-3">
            {(['purchase', 'usage', 'admin', 'refund'] as const).map(type => {
              const txs   = transactions.filter(t => t.tx_type === type)
              const total = txs.reduce((s, t) => s + t.amount, 0)
              return (
                <div key={type} className="rounded-lg bg-slate-800/60 px-3 py-2">
                  <p className="text-xs text-slate-500">{txTypeLabel[type]}</p>
                  <p className={cn('font-semibold text-sm mt-0.5',
                    type === 'usage' ? 'text-red-400' : 'text-emerald-400'
                  )}>
                    {total === 0 ? '—' : `${type === 'usage' ? '-' : '+'}${formatCU(Math.abs(total))}`}
                  </p>
                </div>
              )
            })}
          </div>
        </Card>
      </div>

      {/* Pricing table */}
      {pricing && (
        <Card>
          <CardHeader><CardTitle>Fiyatlandırma</CardTitle></CardHeader>
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-slate-700">
                  <th className="px-4 py-3 text-left font-medium text-slate-400">GPU Tipi</th>
                  <th className="px-4 py-3 text-right font-medium text-slate-400">CU / Saat</th>
                  <th className="px-4 py-3 text-right font-medium text-slate-400">8 Saatlik Örnek</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-slate-700/50">
                {BACKEND_OPTIONS.map(opt => (
                  <tr key={opt.value} className="hover:bg-slate-800/30">
                    <td className="px-4 py-3 text-slate-200 font-medium">{opt.label}</td>
                    <td className="px-4 py-3 text-right text-indigo-300 font-semibold">{opt.rate} CU</td>
                    <td className="px-4 py-3 text-right text-slate-400">{formatCU(opt.rate * 8)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <p className="mt-3 text-xs text-slate-600 px-1">
            * Fiyatlar tahminidir. Gerçek maliyet GPU türüne ve iş süresine göre değişir.
          </p>
        </Card>
      )}

      {/* Transactions */}
      <Card>
        <div className="flex items-center justify-between mb-4">
          <CardTitle>İşlem Geçmişi</CardTitle>
          <div className="flex gap-2">
            {TX_TYPE_FILTERS.map(f => (
              <button
                key={f.value}
                onClick={() => { setTxType(f.value); setPage(0) }}
                className={cn(
                  'rounded-full px-3 py-1 text-xs font-medium transition-colors',
                  txType === f.value
                    ? 'bg-indigo-600 text-white'
                    : 'bg-slate-800 text-slate-400 hover:bg-slate-700'
                )}
              >
                {f.label}
              </button>
            ))}
          </div>
        </div>

        {transactions.length === 0 ? (
          <p className="py-10 text-center text-sm text-slate-500">Henüz işlem bulunmuyor.</p>
        ) : (
          <>
            <div className="overflow-x-auto">
              <table className="w-full text-sm">
                <thead>
                  <tr className="border-b border-slate-700">
                    {['Tür', 'Miktar', 'Açıklama', 'Tarih'].map(h => (
                      <th key={h} className="px-4 py-3 text-left font-medium text-slate-400">{h}</th>
                    ))}
                  </tr>
                </thead>
                <tbody className="divide-y divide-slate-700/50">
                  {transactions.map(tx => (
                    <tr key={tx.id} className="hover:bg-slate-800/30">
                      <td className="px-4 py-3">
                        <span className={cn(
                          'rounded-full px-2 py-0.5 text-xs font-medium',
                          tx.tx_type === 'usage'
                            ? 'bg-red-950/50 text-red-400'
                            : tx.tx_type === 'admin'
                              ? 'bg-indigo-950/50 text-indigo-400'
                              : 'bg-emerald-950/50 text-emerald-400'
                        )}>
                          {txTypeLabel[tx.tx_type] ?? tx.tx_type}
                        </span>
                      </td>
                      <td className={cn(
                        'px-4 py-3 font-semibold',
                        tx.amount < 0 ? 'text-red-400' : 'text-emerald-400'
                      )}>
                        {tx.amount > 0 ? '+' : ''}{formatCU(tx.amount)}
                      </td>
                      <td className="px-4 py-3 text-slate-400">{tx.description ?? '—'}</td>
                      <td className="px-4 py-3 text-slate-500">{formatDate(tx.created_at)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>

            {/* Pagination */}
            <div className="flex items-center justify-between mt-4 pt-4 border-t border-slate-700">
              <p className="text-xs text-slate-500">Sayfa {page + 1}</p>
              <div className="flex gap-2">
                <button
                  onClick={() => setPage(p => Math.max(0, p - 1))}
                  disabled={page === 0}
                  className="flex items-center gap-1 rounded-lg px-3 py-1.5 text-sm text-slate-400 hover:bg-slate-800 disabled:opacity-40"
                >
                  <ChevronLeft className="h-4 w-4" /> Önceki
                </button>
                <button
                  onClick={() => setPage(p => p + 1)}
                  disabled={transactions.length < PAGE_SIZE}
                  className="flex items-center gap-1 rounded-lg px-3 py-1.5 text-sm text-slate-400 hover:bg-slate-800 disabled:opacity-40"
                >
                  Sonraki <ChevronRight className="h-4 w-4" />
                </button>
              </div>
            </div>
          </>
        )}
      </Card>
    </div>
  )
}
