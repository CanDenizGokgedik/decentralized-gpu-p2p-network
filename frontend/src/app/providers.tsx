'use client'

import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { useEffect, useState }              from 'react'
import { ToastProvider }                    from '@/components/ui/Toast'
import { useAuthStore }                     from '@/lib/auth'

/**
 * Ensures _hasHydrated is set to true once the zustand persist middleware
 * finishes reading from localStorage.  This is a belt-and-suspenders measure —
 * the primary signal comes from onRehydrateStorage in auth.ts, but if that
 * fires synchronously before React has mounted this component the flag may
 * already be true; if it fires after mount the subscription below catches it.
 */
function HydrationTrigger() {
  useEffect(() => {
    // If persist already finished (synchronous localStorage read), mark now.
    if (useAuthStore.persist.hasHydrated()) {
      useAuthStore.getState().setHasHydrated(true)
      return
    }
    // Otherwise subscribe to the finish event.
    const unsub = useAuthStore.persist.onFinishHydration(() => {
      useAuthStore.getState().setHasHydrated(true)
    })
    return unsub
  }, [])
  return null
}

export function Providers({ children }: { children: React.ReactNode }) {
  const [queryClient] = useState(
    () =>
      new QueryClient({
        defaultOptions: {
          queries: {
            staleTime:            30_000,
            retry:                1,
            refetchOnWindowFocus: false,
          },
        },
      })
  )

  return (
    <QueryClientProvider client={queryClient}>
      <HydrationTrigger />
      <ToastProvider>{children}</ToastProvider>
    </QueryClientProvider>
  )
}
