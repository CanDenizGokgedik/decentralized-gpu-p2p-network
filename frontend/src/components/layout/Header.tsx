'use client'

import { useQuery } from '@tanstack/react-query'
import { Zap, Menu } from 'lucide-react'
import { useAuth } from '@/lib/auth'
import { computeUnitsApi } from '@/lib/api'
import { formatCU } from '@/lib/utils'
import { Button } from '@/components/ui/Button'

interface HeaderProps {
  onMenuClick?: () => void
}

export function Header({ onMenuClick }: HeaderProps) {
  const { user, logout } = useAuth()

  const { data: balance } = useQuery({
    queryKey: ['balance'],
    queryFn: () => computeUnitsApi.balance().then(r => r.data),
    enabled: !!user,
    refetchInterval: 30_000,
    staleTime: 10_000,
  })

  return (
    <header className="flex h-16 items-center justify-between border-b border-slate-700 bg-slate-900 px-6">
      <div className="flex items-center gap-3">
        <button
          onClick={onMenuClick}
          className="flex lg:hidden items-center justify-center h-9 w-9 rounded-lg text-slate-400 hover:bg-slate-800 hover:text-slate-200 transition-colors"
        >
          <Menu className="h-5 w-5" />
        </button>
        <div className="hidden lg:flex items-center gap-2">
          <Zap className="h-5 w-5 text-indigo-400" />
          <span className="font-semibold text-slate-100">DecentGPU</span>
        </div>
      </div>

      <div className="flex items-center gap-4">
        {balance && (
          <div className="hidden sm:flex items-center gap-2 rounded-lg border border-slate-700 bg-slate-800 px-3 py-1.5 text-sm">
            <span className="text-slate-400">Bakiye:</span>
            <span className="font-semibold text-indigo-300">{formatCU(balance.cu_available)}</span>
          </div>
        )}
        <div className="hidden sm:block truncate max-w-[160px] text-sm text-slate-400">
          {user?.email}
        </div>
        <Button variant="ghost" size="sm" onClick={logout} className="text-slate-400 hover:text-red-400">
          Çıkış
        </Button>
      </div>
    </header>
  )
}
