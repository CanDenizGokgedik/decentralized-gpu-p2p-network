'use client'

import { useState } from 'react'
import { useRouter } from 'next/navigation'
import { useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { z } from 'zod'
import Link from 'next/link'
import { Eye, EyeOff, Cpu, Loader2, AlertCircle } from 'lucide-react'
import { authApi } from '@/lib/api'
import { setToken } from '@/lib/auth'
import { cn } from '@/lib/utils'

const schema = z.object({
  email: z.string().email('Geçerli bir e-posta girin'),
  password: z
    .string()
    .min(8, 'En az 8 karakter')
    .regex(/[A-Z]/, 'Büyük harf içermeli')
    .regex(/[a-z]/, 'Küçük harf içermeli')
    .regex(/[0-9]/, 'Rakam içermeli'),
  role: z.enum(['hirer', 'worker', 'both'], 'Lütfen bir rol seçin'),
})
type FormData = z.infer<typeof schema>

const ROLES = [
  { value: 'hirer',  label: 'İş Veren',  desc: 'GPU kiralar, model eğitir',       icon: '💻' },
  { value: 'worker', label: 'İşçi',       desc: "GPU'sunu kiraya verir",            icon: '🔧' },
  { value: 'both',   label: 'Her İkisi',  desc: 'Hem kiralar hem kiraya verir',     icon: '⚡' },
] as const

function passwordStrength(pw: string) {
  if (pw.length === 0) return { level: 0, label: '', color: '' }
  const checks = [/[A-Z]/.test(pw), /[a-z]/.test(pw), /[0-9]/.test(pw), pw.length >= 8]
  const score = checks.filter(Boolean).length
  if (score <= 1) return { level: 1, label: 'Zayıf',  color: 'bg-red-500' }
  if (score === 2) return { level: 2, label: 'Orta',   color: 'bg-amber-500' }
  if (score === 3) return { level: 3, label: 'İyi',    color: 'bg-yellow-400' }
  return             { level: 4, label: 'Güçlü',  color: 'bg-emerald-500' }
}

export default function RegisterPage() {
  const router = useRouter()
  const [showPw, setShowPw]     = useState(false)
  const [serverErr, setServerErr] = useState('')
  const [loading, setLoading]   = useState(false)

  const {
    register,
    handleSubmit,
    watch,
    setValue,
    formState: { errors },
  } = useForm<FormData>({ resolver: zodResolver(schema) })

  const pw       = watch('password') ?? ''
  const role     = watch('role')
  const strength = passwordStrength(pw)

  const onSubmit = async (data: FormData) => {
    setServerErr('')
    setLoading(true)
    try {
      console.log('register payload:', JSON.stringify(data))
      const res = await authApi.register(data)
      setToken(res.data.token)
      router.push('/dashboard')
    } catch (err: unknown) {
      const e = err as { response?: { data?: { error?: string }; status?: number } }
      console.error('register error:', e?.response?.status, e?.response?.data)
      setServerErr(e?.response?.data?.error ?? 'Kayıt başarısız. Sunucu bağlantısını kontrol edin.')
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

        <h1 className="text-xl font-bold text-slate-100 mb-6 text-center">Hesap Oluştur</h1>

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
                type={showPw ? 'text' : 'password'}
                autoComplete="new-password"
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
                onClick={() => setShowPw(v => !v)}
                className="absolute right-3 top-1/2 -translate-y-1/2 text-slate-400 hover:text-slate-200"
              >
                {showPw ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
              </button>
            </div>
            {/* Strength bar */}
            {pw.length > 0 && (
              <div className="mt-2">
                <div className="flex gap-1 h-1">
                  {[1,2,3,4].map(i => (
                    <div key={i} className={cn(
                      'flex-1 rounded-full transition-all duration-300',
                      i <= strength.level ? strength.color : 'bg-slate-700'
                    )} />
                  ))}
                </div>
                <p className={cn('text-xs mt-1', strength.level >= 3 ? 'text-emerald-400' : strength.level >= 2 ? 'text-amber-400' : 'text-red-400')}>
                  {strength.label}
                </p>
              </div>
            )}
            {errors.password && <p className="mt-1.5 text-xs text-red-400">{errors.password.message}</p>}
          </div>

          {/* Role */}
          <div>
            <label className="block text-sm font-medium text-slate-300 mb-2">Rol Seçin</label>
            <div className="grid grid-cols-3 gap-2">
              {ROLES.map(r => (
                <button
                  key={r.value}
                  type="button"
                  onClick={() => setValue('role', r.value, { shouldValidate: true })}
                  className={cn(
                    'flex flex-col items-center gap-1 rounded-xl border p-3 text-center transition-all text-sm',
                    role === r.value
                      ? 'border-indigo-500 bg-indigo-950/40 text-indigo-300'
                      : 'border-slate-700 bg-slate-800 text-slate-400 hover:border-slate-600 hover:text-slate-200'
                  )}
                >
                  <span className="text-xl">{r.icon}</span>
                  <span className="font-medium text-xs">{r.label}</span>
                  <span className="text-xs opacity-70 leading-tight hidden sm:block">{r.desc}</span>
                </button>
              ))}
            </div>
            {errors.role && <p className="mt-1.5 text-xs text-red-400">{errors.role.message}</p>}
          </div>

          <button
            type="submit"
            disabled={loading}
            className="w-full bg-indigo-600 hover:bg-indigo-500 disabled:opacity-50 disabled:cursor-not-allowed text-white rounded-lg px-4 py-2.5 font-medium transition-all flex items-center justify-center gap-2"
          >
            {loading && <Loader2 className="h-4 w-4 animate-spin" />}
            {loading ? 'Kayıt Olunuyor…' : 'Kayıt Ol'}
          </button>

          <p className="text-center text-sm text-slate-400">
            Zaten hesabınız var mı?{' '}
            <Link href="/login" className="text-indigo-400 hover:text-indigo-300 font-medium">Giriş Yap</Link>
          </p>
        </form>
      </div>
    </div>
  )
}
