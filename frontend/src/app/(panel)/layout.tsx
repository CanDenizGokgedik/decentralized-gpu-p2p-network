'use client'

import { useEffect, useState } from 'react'
import { useRouter }           from 'next/navigation'
import { useAuthStore }        from '@/lib/auth'
import { Sidebar }             from '@/components/layout/Sidebar'
import { Header }              from '@/components/layout/Header'
import { MobileNav }           from '@/components/layout/MobileNav'

export default function PanelLayout({ children }: { children: React.ReactNode }) {
  const { user, _hasHydrated } = useAuthStore()
  const router                 = useRouter()
  const [mobileOpen, setMobileOpen] = useState(false)
  // Prevent SSR/hydration mismatch — only evaluate auth after mount.
  const [mounted, setMounted] = useState(false)
  useEffect(() => { setMounted(true) }, [])

  useEffect(() => {
    // Wait for both DOM mount and zustand rehydration before deciding to redirect.
    if (!mounted || !_hasHydrated) return
    if (!user) router.push('/login')
  }, [user, _hasHydrated, mounted, router])

  // ── Loading state — show spinner until localStorage is read ─────────────────
  if (!mounted || !_hasHydrated) {
    return (
      <div className="min-h-screen bg-slate-950 flex items-center justify-center">
        <div className="flex flex-col items-center gap-3">
          <div className="w-8 h-8 border-2 border-indigo-500 border-t-transparent rounded-full animate-spin" />
          <p className="text-slate-500 text-sm">Yükleniyor…</p>
        </div>
      </div>
    )
  }

  // ── Redirecting — render nothing to avoid layout flash ──────────────────────
  if (!user) return null

  // ── Authenticated ────────────────────────────────────────────────────────────
  return (
    <div className="flex h-screen overflow-hidden bg-slate-950">
      {/* Desktop sidebar */}
      <div className="hidden lg:flex">
        <Sidebar />
      </div>

      {/* Mobile nav overlay */}
      <MobileNav open={mobileOpen} onClose={() => setMobileOpen(false)} />

      {/* Main content */}
      <div className="flex flex-1 flex-col overflow-hidden">
        <Header onMenuClick={() => setMobileOpen(true)} />
        <main className="flex-1 overflow-y-auto p-6">
          {children}
        </main>
      </div>
    </div>
  )
}
