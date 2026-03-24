'use client'

import { useState } from 'react'
import Link from 'next/link'
import { useRouter } from 'next/navigation'
import { useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { z } from 'zod'
import { Eye, EyeOff, Cpu, Loader2, AlertCircle } from 'lucide-react'
import { authApi } from '@/lib/api'
import { setToken } from '@/lib/auth'
import { cn } from '@/lib/utils'

const schema = z.object({
  email:    z.string().email('Geçerli bir e-posta adresi girin'),
  password: z.string().min(1, 'Şifre zorunludur'),
})
type FormValues = z.infer<typeof schema>

export default function LoginPage() {
  const router            = useRouter()
  const [show, setShow]   = useState(false)
  const [serverErr, setServerErr] = useState('')
  const [loading, setLoading]     = useState(false)

  const { register, handleSubmit, formState: { errors } } = useForm<FormValues>({
    resolver: zodResolver(schema),
  })

  const onSubmit = async (data: FormValues) => {
    setServerErr('')
    setLoading(true)
    try {
      console.log('login payload:', JSON.stringify({ email: data.email }))
      const res = await authApi.login(data)
      setToken(res.data.token)
      router.push('/dashboard')
    } catch (err: unknown) {
      const e = err as { response?: { data?: { error?: string }; status?: number } }
      console.error('login error:', e?.response?.status, e?.response?.data)
      setServerErr(e?.response?.data?.error ?? 'Giriş başarısız. E-posta veya şifrenizi kontrol edin.')
    } finally {
      setLoading(false)
    }
  }

  return (
    <div className="min-h-screen bg-slate-950 flex items-center justify-center p-4"
         style={{ background: 'radial-gradient(ellipse at top, #1e1b4b 0%, transparent 60%), #020617' }}>
      <div className="w-full max-w-md bg-slate-900 border border-slate-800 rounded-2xl p-8 shadow-2xl">

        {/* Logo */}
        <div className="text-center mb-8">
          <div className="inline-flex items-center gap-2 mb-2">
            <div className="w-8 h-8 bg-indigo-500 rounded-lg flex items-center justify-center">
              <Cpu className="w-5 h-5 text-white" />
            </div>
            <span className="text-xl font-bold text-slate-100">DecentGPU</span>
          </div>
          <p className="text-slate-400 text-sm">Dağıtık GPU Kiralama Platformu</p>
        </div>

        <h1 className="text-xl font-bold text-slate-100 mb-6 text-center">Giriş Yap</h1>

        {serverErr && (
          <div className="mb-4 flex items-start gap-3 rounded-lg border border-red-500/30 bg-red-950/30 px-4 py-3">
            <AlertCircle className="h-5 w-5 text-red-400 shrink-0 mt-0.5" />
            <p className="text-sm text-red-300">{serverErr}</p>
          </div>
        )}

        <form onSubmit={handleSubmit(onSubmit)} className="space-y-5">
          {/* Email */}
          <div>
            <label className="block text-sm font-medium text-slate-300 mb-1.5">E-posta</label>
            <input
              {...register('email')}
              type="email"
              autoComplete="email"
              placeholder="ornek@email.com"
              className={cn(
                'w-full bg-slate-800 border rounded-lg px-4 py-2.5 text-slate-100 placeholder:text-slate-500',
                'focus:outline-none focus:ring-2 focus:border-transparent transition-all',
                errors.email
                  ? 'border-red-500 focus:ring-red-500'
                  : 'border-slate-700 focus:ring-indigo-500'
              )}
            />
            {errors.email && <p className="mt-1.5 text-xs text-red-400">{errors.email.message}</p>}
          </div>

          {/* Password */}
          <div>
            <label className="block text-sm font-medium text-slate-300 mb-1.5">Şifre</label>
            <div className="relative">
              <input
                {...register('password')}
                type={show ? 'text' : 'password'}
                autoComplete="current-password"
                placeholder="••••••••"
                className={cn(
                  'w-full bg-slate-800 border rounded-lg px-4 py-2.5 pr-10 text-slate-100 placeholder:text-slate-500',
                  'focus:outline-none focus:ring-2 focus:border-transparent transition-all',
                  errors.password
                    ? 'border-red-500 focus:ring-red-500'
                    : 'border-slate-700 focus:ring-indigo-500'
                )}
              />
              <button
                type="button"
                onClick={() => setShow(s => !s)}
                className="absolute right-3 top-1/2 -translate-y-1/2 text-slate-400 hover:text-slate-200"
              >
                {show ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
              </button>
            </div>
            {errors.password && <p className="mt-1.5 text-xs text-red-400">{errors.password.message}</p>}
          </div>

          <button
            type="submit"
            disabled={loading}
            className="w-full bg-indigo-600 hover:bg-indigo-500 disabled:opacity-50 disabled:cursor-not-allowed text-white rounded-lg px-4 py-2.5 font-medium transition-all flex items-center justify-center gap-2"
          >
            {loading && <Loader2 className="h-4 w-4 animate-spin" />}
            {loading ? 'Giriş Yapılıyor…' : 'Giriş Yap'}
          </button>

          <p className="text-center text-sm text-slate-400">
            Hesabınız yok mu?{' '}
            <Link href="/register" className="text-indigo-400 hover:text-indigo-300 font-medium">Kayıt Olun</Link>
          </p>
        </form>
      </div>
    </div>
  )
}
