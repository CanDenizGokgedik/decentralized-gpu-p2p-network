'use client'

import { create }    from 'zustand'
import { persist, createJSONStorage } from 'zustand/middleware'
import { useEffect } from 'react'
import { useRouter } from 'next/navigation'
import type { UserClaims } from '@/types'
import { TOKEN_KEY } from './constants'

// ── JWT parsing (client-side, no verification) ────────────────────────────────

function parseJwt(token: string): UserClaims | null {
  try {
    const parts = token.split('.')
    if (parts.length !== 3) return null
    // Fix base64url → base64 padding
    const b64 = parts[1].replace(/-/g, '+').replace(/_/g, '/')
    const payload = JSON.parse(atob(b64))
    // Allow 60-second clock skew before treating as expired
    if (payload.exp && payload.exp * 1000 < Date.now() - 60_000) {
      console.log('[auth] token expired, clearing')
      return null
    }
    return payload as UserClaims
  } catch (e) {
    console.error('[auth] failed to parse JWT:', e)
    return null
  }
}

// ── Zustand store with localStorage persistence ───────────────────────────────

interface AuthStore {
  user:           UserClaims | null
  token:          string | null
  isLoading:      boolean
  /** True once the persist middleware has finished reading from localStorage. */
  _hasHydrated:   boolean
  setHasHydrated: (val: boolean) => void
  login:          (token: string) => void
  logout:         () => void
}

export const useAuthStore = create<AuthStore>()(
  persist(
    (set) => ({
      user:           null,
      token:          null,
      isLoading:      false,
      _hasHydrated:   false,

      setHasHydrated: (val: boolean) => set({ _hasHydrated: val }),

      login: (token: string) => {
        const user = parseJwt(token)
        if (!user) {
          console.error('[auth] login: invalid or expired token')
          return
        }
        console.log('[auth] login:', user.email, 'role:', user.role)
        // Keep the legacy TOKEN_KEY in sync for api.ts getToken() fallback
        if (typeof window !== 'undefined') {
          localStorage.setItem(TOKEN_KEY, token)
        }
        set({ user, token, isLoading: false })
      },

      logout: () => {
        console.log('[auth] logout')
        if (typeof window !== 'undefined') {
          localStorage.removeItem(TOKEN_KEY)
        }
        set({ user: null, token: null, isLoading: false })
      },
    }),
    {
      // Use v2 key so any corrupt v1 state in localStorage is ignored.
      name:    'decentgpu_auth_v2',
      storage: createJSONStorage(() => localStorage),
      // Only persist user-facing data, not internal flags.
      partialize: (state) => ({
        token: state.token,
        user:  state.user,
      }),
      onRehydrateStorage: () => (state, error) => {
        if (error) {
          console.error('[auth] rehydration error:', error)
        }
        if (state) {
          // Re-validate stored token in case it expired while the tab was closed.
          if (state.token) {
            const user = parseJwt(state.token)
            if (!user) {
              console.log('[auth] stored token expired during rehydration, clearing')
              state.token = null
              state.user  = null
              if (typeof window !== 'undefined') {
                localStorage.removeItem(TOKEN_KEY)
              }
            } else {
              state.user = user
              // Keep legacy key in sync
              if (typeof window !== 'undefined') {
                localStorage.setItem(TOKEN_KEY, state.token)
              }
            }
          }
          state.isLoading    = false
          state._hasHydrated = true
        }
      },
    }
  )
)

// ── Token helpers (non-hook, safe to call anywhere) ───────────────────────────

export function getToken(): string | null {
  if (typeof window === 'undefined') return null
  const storeToken = useAuthStore.getState().token
  if (storeToken) return storeToken
  // Legacy fallback for code that writes directly to localStorage
  return localStorage.getItem(TOKEN_KEY)
}

export function setToken(token: string): void {
  useAuthStore.getState().login(token)
}

export function clearToken(): void {
  useAuthStore.getState().logout()
}

export function getUserFromToken(token: string | null): UserClaims | null {
  if (!token) return null
  return parseJwt(token)
}

// ── Hooks ─────────────────────────────────────────────────────────────────────

export function useAuth() {
  const { user, token, isLoading, login, logout } = useAuthStore()
  return { user, token, isLoading, login, logout, isLoggedIn: !!user }
}

/**
 * Redirects to /login if unauthenticated.
 * Returns null while hydrating from localStorage (prevents flash redirect).
 * Returns null while redirecting.
 * Returns the user once auth is confirmed.
 */
export function useRequireAuth() {
  const { user, _hasHydrated } = useAuthStore()
  const router = useRouter()

  useEffect(() => {
    // Don't redirect until we've read localStorage — avoids a flash redirect
    // on every page refresh when the token is actually valid.
    if (!_hasHydrated) return
    if (!user) {
      router.replace('/login')
    }
  }, [user, _hasHydrated, router])

  // Returning null here tells the layout to render a loading/empty state.
  if (!_hasHydrated) return null
  return user
}

export function useRequireAdmin() {
  const { user, _hasHydrated } = useAuthStore()
  const router = useRouter()

  useEffect(() => {
    if (!_hasHydrated) return
    if (!user) {
      router.replace('/login')
      return
    }
    if (user.role !== 'admin') {
      router.replace('/dashboard')
    }
  }, [user, _hasHydrated, router])

  if (!_hasHydrated) return null
  if (!user || user.role !== 'admin') return null
  return user
}

// Kept for backward compatibility
export function useAuthInit() {
  // no-op — persist middleware handles rehydration
}
