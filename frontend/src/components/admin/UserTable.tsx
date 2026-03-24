'use client'

import { useState } from 'react'
import { useMutation, useQueryClient } from '@tanstack/react-query'
import { adminApi } from '@/lib/api'
import { Badge } from '@/components/ui/Badge'
import { Button } from '@/components/ui/Button'
import { useToast } from '@/components/ui/Toast'
import { AllocateCUModal } from './AllocateCUModal'
import { formatCU, formatDate, shorten } from '@/lib/utils'
import { Coins, Shield } from 'lucide-react'
import { cn } from '@/lib/utils'
import type { AdminUser } from '@/types'

const ROLE_OPTIONS = ['client', 'worker', 'admin'] as const
type Role = typeof ROLE_OPTIONS[number]

const roleVariant: Record<Role, 'default' | 'success' | 'warning' | 'danger' | 'info'> = {
  client: 'default',
  worker: 'info',
  admin:  'warning',
}

interface UserTableProps {
  users: AdminUser[]
}

export function UserTable({ users }: UserTableProps) {
  const { toast }      = useToast()
  const queryClient    = useQueryClient()
  const [allocTarget, setAllocTarget] = useState<AdminUser | null>(null)

  const roleMut = useMutation({
    mutationFn: ({ id, role }: { id: string; role: string }) => adminApi.updateRole(id, role),
    onSuccess: () => {
      toast('Rol güncellendi.', 'success')
      queryClient.invalidateQueries({ queryKey: ['admin-users'] })
    },
    onError: (err: unknown) => {
      const msg = (err as { response?: { data?: { error?: string } } })?.response?.data?.error
      toast(msg ?? 'Rol güncellenemedi.', 'error')
    },
  })

  return (
    <>
      <div className="overflow-x-auto">
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b border-slate-700">
              {['ID', 'E-posta', 'Rol', 'CU Bakiyesi', 'Kayıt', 'İşlemler'].map(h => (
                <th key={h} className="px-4 py-3 text-left font-medium text-slate-400">{h}</th>
              ))}
            </tr>
          </thead>
          <tbody className="divide-y divide-slate-700/50">
            {users.map(user => (
              <tr key={user.id} className="hover:bg-slate-800/30 transition-colors">
                <td className="px-4 py-3 font-mono text-xs text-slate-400">{shorten(user.id, 8)}</td>
                <td className="px-4 py-3 text-slate-200">{user.email}</td>
                <td className="px-4 py-3">
                  <select
                    value={user.role}
                    onChange={e => roleMut.mutate({ id: user.id, role: e.target.value })}
                    className="rounded-lg border border-slate-700 bg-slate-900 px-2 py-1 text-xs text-slate-200 focus:outline-none focus:ring-2 focus:ring-indigo-500"
                  >
                    {ROLE_OPTIONS.map(r => (
                      <option key={r} value={r}>{r}</option>
                    ))}
                  </select>
                </td>
                <td className="px-4 py-3 text-indigo-300 font-medium">
                  {user.cu_balance != null ? formatCU(user.cu_balance) : '—'}
                </td>
                <td className="px-4 py-3 text-slate-500 text-xs">{formatDate(user.created_at)}</td>
                <td className="px-4 py-3">
                  <Button
                    size="sm"
                    variant="ghost"
                    onClick={() => setAllocTarget(user)}
                  >
                    <Coins className="h-3.5 w-3.5" />
                    CU Ver
                  </Button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      {allocTarget && (
        <AllocateCUModal
          open={!!allocTarget}
          onClose={() => setAllocTarget(null)}
          userId={allocTarget.id}
          userEmail={allocTarget.email}
        />
      )}
    </>
  )
}
