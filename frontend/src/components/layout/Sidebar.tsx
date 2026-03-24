'use client'

import Link from 'next/link'
import { usePathname } from 'next/navigation'
import {
  Home, Cpu, ClipboardList, Wrench, Coins,
  Settings, Download, Shield, LogOut, Zap, Code2,
} from 'lucide-react'
import { cn } from '@/lib/utils'
import { useAuth } from '@/lib/auth'

const iconMap: Record<string, React.ComponentType<{ className?: string }>> = {
  Home, Cpu, ClipboardList, Wrench, Coins, Settings, Download, Shield, Code2,
}

const navItems = [
  { href: '/dashboard',        label: 'Genel Bakış',   icon: 'Home',          roles: ['hirer','worker','both','admin'] },
  { href: '/rent',             label: 'GPU Kirala',    icon: 'Cpu',           roles: ['hirer','worker','both','admin'] },
  { href: '/jobs',             label: 'İşlerim',       icon: 'ClipboardList', roles: ['hirer','worker','both','admin'] },
  { href: '/editor',           label: 'Kod Editörü',   icon: 'Code2',         roles: ['hirer','both','admin'] },
  { href: '/worker-dashboard', label: 'Worker Paneli',  icon: 'Wrench',        roles: ['worker','both'] },
  { href: '/compute-units',    label: 'Compute Units', icon: 'Coins',         roles: ['hirer','worker','both','admin'] },
  { href: '/account',          label: 'Hesabım',       icon: 'Settings',      roles: ['hirer','worker','both','admin'] },
  { href: '/account/client',   label: 'İstemci İndir', icon: 'Download',      roles: ['hirer','worker','both','admin'] },
  { href: '/admin',            label: 'Yönetim',       icon: 'Shield',        roles: ['admin'] },
]

export function Sidebar() {
  const pathname      = usePathname()
  const { user, logout } = useAuth()

  const visible = navItems.filter(item =>
    !user || item.roles.includes(user.role)
  )

  return (
    <aside className="flex h-full w-64 flex-col border-r border-slate-700 bg-slate-900">
      {/* Logo */}
      <div className="flex h-16 items-center gap-2 border-b border-slate-700 px-6">
        <Zap className="h-6 w-6 text-indigo-400" />
        <span className="text-lg font-bold text-slate-100">DecentGPU</span>
      </div>

      {/* Nav */}
      <nav className="flex-1 overflow-y-auto py-4 px-3">
        <ul className="space-y-1">
          {visible.map(item => {
            const Icon   = iconMap[item.icon]
            const active = pathname === item.href || (item.href !== '/dashboard' && pathname.startsWith(item.href))
            return (
              <li key={item.href}>
                <Link
                  href={item.href}
                  className={cn(
                    'flex items-center gap-3 rounded-lg px-3 py-2.5 text-sm font-medium transition-colors',
                    active
                      ? 'bg-indigo-600/20 text-indigo-300'
                      : 'text-slate-400 hover:bg-slate-800 hover:text-slate-200'
                  )}
                >
                  {Icon && <Icon className="h-5 w-5 shrink-0" />}
                  {item.label}
                </Link>
              </li>
            )
          })}
        </ul>
      </nav>

      {/* User footer */}
      <div className="border-t border-slate-700 p-4">
        <div className="mb-2 truncate text-xs text-slate-400">{user?.email}</div>
        <button
          onClick={logout}
          className="flex w-full items-center gap-2 rounded-lg px-3 py-2 text-sm text-slate-400 hover:bg-slate-800 hover:text-red-400 transition-colors"
        >
          <LogOut className="h-4 w-4" />
          Çıkış Yap
        </button>
      </div>
    </aside>
  )
}
